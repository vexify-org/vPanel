//! 手写、极简的 HTTP/1.1 服务器。
//!
//! 刻意不引入 tokio/hyper，只用 std 完成：一个接受线程 + 固定工作线程池。
//! 每处理完一个请求即关闭连接，避免长连接持有额外缓冲，把内存占用钉在有界范围内。
//!
//! 支持：
//!   GET /          -> 面板 HTML
//!   GET /health    -> "ok"
//!   GET /metrics   -> 纯文本状态（请求数 / 并发 / RSS）
//!   其余           -> 404

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::config::Config;

const MAX_REQ: usize = 8192;

/// 线程间共享的状态与统计。
pub struct State {
    pub started: Instant,
    pub requests: AtomicU64,
    pub active: AtomicU64,
    pub conns: AtomicU64,
    pub cfg: Config,
    pub monitor: Arc<crate::system::Monitor>,
    pub shop: Arc<crate::shop::Shop>,
    pub plugins: Arc<crate::plugins::Plugins>,
}

/// 启动监听并派发工作线程。阻塞运行，直到进程退出。
pub fn serve(cfg: Config) -> std::io::Result<()> {
    let monitor = crate::system::Monitor::start();
    let shop = crate::shop::Shop::new();
    let plugins = crate::plugins::Plugins::new();
    plugins.load(&cfg); // 从 plugins 目录加载插件 + 启动定时线程
    let addr = format!("{}:{}", cfg.server.bind, cfg.server.port);
    let listener = TcpListener::bind(&addr)?;
    listener.set_nonblocking(true)?;

    let state = Arc::new(State {
        started: Instant::now(),
        requests: AtomicU64::new(0),
        active: AtomicU64::new(0),
        conns: AtomicU64::new(0),
        cfg,
        monitor,
        shop,
        plugins,
    });

    // 有界队列：高并发时连接在此排队或直接拒绝，内存不随之膨胀。
    let queue: Arc<Mutex<std::collections::VecDeque<TcpStream>>> =
        Arc::new(Mutex::new(std::collections::VecDeque::new()));
    let cap = state.cfg.server.backlog.max(16);

    // 固定工作线程池。
    let workers = state.cfg.server.workers.max(1);
    for _ in 0..workers {
        let queue = queue.clone();
        let state = state.clone();
        std::thread::spawn(move || worker(queue, state));
    }

    eprintln!("panel listening on http://{}/", addr);
    accept_loop(&listener, queue, cap, state);

    Ok(())
}

/// 接受线程：非阻塞 accept，把新连接放入有界队列。
fn accept_loop(
    listener: &TcpListener,
    queue: Arc<Mutex<std::collections::VecDeque<TcpStream>>>,
    cap: usize,
    state: Arc<State>,
) {
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                state.conns.fetch_add(1, Ordering::Relaxed);
                let mut q = queue.lock().unwrap();
                if q.len() >= cap {
                    // 队列已满：丢弃，让内核背压，保持内存有界。
                    let _ = stream;
                } else {
                    let _ = stream.set_nodelay(true);
                    q.push_back(stream);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(_) => {}
        }
    }
}

/// 工作线程：从队列取出连接并处理。
fn worker(queue: Arc<Mutex<std::collections::VecDeque<TcpStream>>>, state: Arc<State>) {
    loop {
        let stream = {
            let mut q = queue.lock().unwrap();
            q.pop_front()
        };
        match stream {
            Some(mut s) => {
                state.active.fetch_add(1, Ordering::Relaxed);
                handle(&mut s, &state);
                state.active.fetch_sub(1, Ordering::Relaxed);
            }
            None => std::thread::sleep(std::time::Duration::from_millis(1)),
        }
    }
}

fn handle(stream: &mut TcpStream, state: &State) {
    let mut buf = [0u8; MAX_REQ];
    // 尽力读取请求头；非阻塞超时的读失败直接忽略。
    let n = read_head(stream, &mut buf);

    state.requests.fetch_add(1, Ordering::Relaxed);

    // 只解析请求行，取方法 + 路径。
    let head = String::from_utf8_lossy(&buf[..n.min(MAX_REQ)]);
    let line = head.lines().next().unwrap_or("");
    let mut wt = line.split_whitespace();
    let method = wt.next().unwrap_or("GET").to_string();
    let target = wt.next().unwrap_or("/");

    // 插件事件钩子：每个进入的 HTTP 请求（慎用，脚本里勿放慢操作）。
    state.plugins.run_hooks("on_http_request");

    // Web Socket 与会话式终端：升级为长连接并驱动 PTY。
    if target == "/ws" {
        if let Ok(clone) = stream.try_clone() {
            state.active.fetch_add(1, Ordering::Relaxed);
            if let Some(ws) = crate::ws::Ws::accept(clone, &head) {
                crate::term::run(ws, &state.cfg.shell);
            }
            state.active.fetch_sub(1, Ordering::Relaxed);
        }
        return;
    }

    // POST 请求体（按 Content-Length 读取）。
    let body = read_body(stream, &head, &buf[..n]);

    // MCP 端点：供 AI 客户端调用面板能力（JSON-RPC over HTTP）。
    if target == "/mcp" && method == "POST" {
        if let Ok(mut clone) = stream.try_clone() {
            let resp = crate::mcp::handle(&body, state);
            let _ = respond(&mut clone, "200 OK", "application/json; charset=utf-8", &resp);
        }
        return;
    }

    // /api/* 端点：JSON 数据或操作。
    if target.starts_with("/api/") {
        let resp = crate::api::route(&method, target, &body, state);
        let _ = respond(stream, "200 OK", "application/json; charset=utf-8", &resp);
        return;
    }

    let (status, ctype, body): (&str, &str, Vec<u8>) = match target {
        "/" | "/index.html" => {
            let html = crate::panel::render(state);
            ("200 OK", "text/html; charset=utf-8", html.into_bytes())
        }
        "/term" | "/term.html" => {
            let html = crate::panel::render_term(&state.cfg);
            ("200 OK", "text/html; charset=utf-8", html.into_bytes())
        }
        "/health" => ("200 OK", "text/plain; charset=utf-8", b"ok\n".to_vec()),
        "/metrics" => ("200 OK", "text/plain; charset=utf-8",
            format!(
                "requests {}\nactive {}\nconns {}\nrss_kb {}\nuptime_s {}\n",
                state.requests.load(Ordering::Relaxed),
                state.active.load(Ordering::Relaxed),
                state.conns.load(Ordering::Relaxed),
                crate::panel::rss_kb(),
                state.started.elapsed().as_secs(),
            ).into_bytes()),
        "/favicon.ico" => ("204 No Content", "text/plain", Vec::new()),
        _ => ("404 Not Found", "text/plain; charset=utf-8", b"not found\n".to_vec()),
    };

    let _ = respond(stream, status, ctype, &body);
    // 处理完即关闭连接。
}

/// 读取请求头之后的请求体：先取缓冲区剩余，再按 Content-Length 补充。
fn read_body(stream: &mut TcpStream, head: &str, buf: &[u8]) -> Vec<u8> {
    let clen: usize = head
        .split("\r\n")
        .find_map(|l| {
            let lower = l.to_ascii_lowercase();
            if lower.starts_with("content-length:") {
                lower
                    .split_once(':')
                    .and_then(|(_, v)| v.trim().parse().ok())
            } else {
                None
            }
        })
        .unwrap_or(0);
    if clen == 0 {
        return Vec::new();
    }
    // 头结束位置。
    let head_end = match head.find("\r\n\r\n") {
        Some(i) => i + 4,
        None => return Vec::new(),
    };
    let mut body = buf[head_end.min(buf.len())..buf.len().min(head_end + clen)].to_vec();
    while body.len() < clen {
        let mut t = [0u8; 2048];
        match stream.read(&mut t) {
            Ok(0) | Err(_) => break,
            Ok(m) => body.extend_from_slice(&t[..m]),
        }
    }
    body.truncate(clen);
    body
}

/// 只读取最多 MAX_REQ 字节（HTTP 请求头通常足够）。
fn read_head(stream: &mut TcpStream, buf: &mut [u8]) -> usize {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    let mut total = 0;
    while total < buf.len() {
        match stream.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(m) => {
                total += m;
                // 发现空行（\r\n\r\n）表示请求头结束。
                if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    total
}

#[allow(clippy::too_many_arguments)]
fn respond(stream: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        ctype,
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}