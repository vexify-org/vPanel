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

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::config::Config;
use crate::tls::Io;

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
    pub auth: Arc<crate::auth::SecurityGuard>,
    pub iota: Arc<crate::iota::Manager>,
    pub tls: crate::tls::Server,
}

/// 启动监听并派发工作线程。阻塞运行，直到进程退出。
pub fn serve(cfg: Config) -> std::io::Result<()> {
    let monitor = crate::system::Monitor::start();
    crate::monitor::start();
    let shop = crate::shop::Shop::new();
    let plugins = crate::plugins::Plugins::new();
    plugins.load(&cfg); // 从 plugins 目录加载插件 + 启动定时线程
    crate::api::db_config_init(&cfg);
    crate::api::certs_config_init(&cfg);
    crate::api::backup_config_init(&cfg);
    crate::api::config_init(&cfg);
    let auth = Arc::new(crate::auth::SecurityGuard::new(cfg.security.clone()));
    let iota = crate::iota::Manager::load(cfg.iota.clone());
    let tls = crate::tls::Server::build(&cfg.server.tls)?;
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
        auth,
        iota,
        tls,
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

    eprintln!(
        "panel listening on {}://{}/",
        if state.tls.enabled() { "https" } else { "http" },
        addr
    );
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
        if let Some(raw) = stream {
            state.active.fetch_add(1, Ordering::Relaxed);
            // TLS 使能时先握手，再进入统一的 HTTP 处理。
            if let Ok(mut conn) = crate::tls::accept(raw, &state.tls) {
                handle(&mut *conn, &state);
            }
            state.active.fetch_sub(1, Ordering::Relaxed);
        } else {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

fn handle(stream: &mut dyn Io, state: &State) {
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

    // 解析 Cookie / User-Agent / 来源 IP。
    let cookie = header_val(&head, "cookie");
    let ua = header_val(&head, "user-agent").unwrap_or("ua");
    let client_key = stream.peer_ip().unwrap_or_else(|| "local".to_string());
    let authed = state.auth.validate(cookie);
    let _secure = state.cfg.security.trust_proxy
        && header_val(&head, "x-forwarded-proto").map(|p| p.eq_ignore_ascii_case("https")).unwrap_or(false);

    // 读取请求体（POST 才有；GET / WS 请求头内为空，安全）。
    let body = read_body(stream, &head, &buf[..n]);

    // MCP 端点：会话 cookie 或独立 Bearer 令牌二选一放行。
    if target == "/mcp" && method == "POST" {
        let bearer_ok = header_val(&head, "authorization")
            .map(|a| {
                let token = a.trim_start_matches("Bearer ").trim().to_string();
                !token.is_empty() && token == state.cfg.security.mcp_token
            })
            .unwrap_or(false);
        if !state.auth.enabled() || authed || bearer_ok {
            if let Some(mut clone) = stream.dup() {
                let resp = crate::mcp::handle(&body, state);
                let _ = respond(&mut *clone, "200 OK", "application/json; charset=utf-8", &resp);
            }
        } else {
            let _ = respond(stream, "401 Unauthorized", "application/json; charset=utf-8", "{\"ok\":false,\"msg\":\"未授权\"}".as_bytes());
        }
        return;
    }

    // 认证端点（无需会话即可访问：登录/初始设置/退出/认证状态）。
    if state.auth.enabled() {
        if let Some(resp) = auth_endpoint(state, &method, target, &body, cookie, ua, &client_key) {
            send_auth(stream, &resp);
            return;
        }
        // 需要登录：拦下所有页面 / API / 终端。
        if !authed {
            if target == "/ws" || target.starts_with("/api/") || target.starts_with("/api") {
                let _ = respond(stream, "401 Unauthorized", "application/json; charset=utf-8", "{\"ok\":false,\"msg\":\"未登录\"}".as_bytes());
            } else {
                let html = crate::panel::render_login(state);
                let _ = respond(stream, "200 OK", "text/html; charset=utf-8", html.as_bytes());
            }
            return;
        }
        // 已登录：处理需要改 cookie 的管理端认证操作（改密/会话管理）。
        if target.starts_with("/api/auth/") {
            if let Some(resp) = authenticated_admin_endpoint(state, &method, target, &body, cookie, ua) {
                send_auth(stream, &resp);
                return;
            }
        }
    }

    // Web Socket 与会话式终端：升级为长连接并驱动 PTY。
    if target == "/ws" {
        if let Some(conn) = stream.dup() {
            state.active.fetch_add(1, Ordering::Relaxed);
            if let Some(ws) = crate::ws::Ws::accept(conn, &head) {
                crate::term::run(ws, &state.cfg.shell);
            }
            state.active.fetch_sub(1, Ordering::Relaxed);
        }
        return;
    }

    // 文件下载：原始字节流 + attachment（区别于 JSON API）。
    if target.starts_with("/api/file/download") {
        let path = query_val(target, "path");
        if let Some(p) = path {
            if let Some(bytes) = crate::extra::download(&p) {
                let fname = std::path::Path::new(&p)
                    .file_name()
                    .map(|x| x.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "download".to_string());
                let safe_fname: String = fname
                    .chars()
                    .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
                    .collect();
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"{}\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    safe_fname,
                    bytes.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&bytes);
                let _ = stream.flush();
            } else {
                let _ = respond(stream, "404 Not Found", "text/plain; charset=utf-8", b"file not found\n");
            }
        } else {
            let _ = respond(stream, "400 Bad Request", "text/plain; charset=utf-8", b"missing path\n");
        }
        return;
    }

    // IotaPanel 兼容网关：`<prefix>/<name>/*` 反向代理到插件进程端口（冷启动 + WS 透传）。
    {
        let prefix = &state.cfg.iota.prefix;
        let is_prefix = target.starts_with(prefix.as_str())
            && (target.len() == prefix.len() || target.as_bytes().get(prefix.len()).copied() == Some(b'/'));
        if is_prefix {
            // 请求头 `\r\n\r\n` 之后的多余字节（升级请求可能带早期帧）一并透传。
            let raw = &buf[..n.min(MAX_REQ)];
            let extra: Vec<u8> = match raw.windows(4).position(|w| w == b"\r\n\r\n") {
                Some(i) => raw[i + 4..].to_vec(),
                None => Vec::new(),
            };
            let resp = crate::iota::gateway_proxy(
                &state.cfg.iota,
                &state.iota,
                &method,
                target,
                &head,
                &body,
                &extra,
                stream,
                state.tls.enabled(),
            );
            // 网关正常时已把响应直接写回 stream（resp 为空）；非空表示需托管回写。
            if !resp.is_empty() {
                let _ = respond(stream, "200 OK", "application/json; charset=utf-8", resp.as_bytes());
            }
            return;
        }
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
fn read_body(stream: &mut dyn Io, head: &str, buf: &[u8]) -> Vec<u8> {
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
fn read_head(stream: &mut dyn Io, buf: &mut [u8]) -> usize {
    stream.set_rto(std::time::Duration::from_secs(5));
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

/// 从 URI 的查询串中按键取值（极简 percent 解码）。
fn query_val(target: &str, key: &str) -> Option<String> {
    let qs = target.split_once('?').map(|(_, q)| q)?;
    for pair in qs.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        if percent_decode(k) == key {
            return Some(percent_decode(v));
        }
    }
    None
}

/// 极简 percent 解码（+ 视为空格）。
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let h = (b[i + 1] as char).to_digit(16);
                let l = (b[i + 2] as char).to_digit(16);
                if let (Some(h), Some(l)) = (h, l) {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                    continue;
                }
                out.push(b[i]);
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[allow(clippy::too_many_arguments)]
fn respond(stream: &mut dyn Io, status: &str, ctype: &str, body: &[u8]) -> std::io::Result<()> {
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

// ---------------------------------------------------------------------------
// 认证端点与辅助
// ---------------------------------------------------------------------------

/// 请求头取值（大小写不敏感），返回首匹配。
fn header_val<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    let lower = name.to_ascii_lowercase();
    head.lines().find_map(|l| {
        let t = l.trim();
        if t.to_ascii_lowercase().starts_with(&(lower.clone() + ":")) {
            Some(t[name.len() + 1..].trim())
        } else {
            None
        }
    })
}

/// 带响应头(Cookie)的通用响应。
fn respond_with_headers(
    stream: &mut dyn Io,
    status: &str,
    ctype: &str,
    body: &[u8],
    set_cookie: Option<&str>,
) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        status,
        ctype,
        body.len()
    );
    if let Some(sc) = set_cookie {
        head.push_str("Set-Cookie: ");
        head.push_str(sc);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// 认证响应：状态 + 可选下发/清除 cookie。
struct AuthResp {
    status: String,
    ctype: String,
    body: Vec<u8>,
    set_cookie: Option<String>,
    clear_cookie: bool,
}

fn auth_json(body: String) -> AuthResp {
    AuthResp {
        status: "200 OK".into(),
        ctype: "application/json; charset=utf-8".into(),
        body: body.into_bytes(),
        set_cookie: None,
        clear_cookie: false,
    }
}

fn auth_err(msg: &str) -> AuthResp {
    auth_json(format!("{{\"ok\":false,\"msg\":\"{}\"}}", crate::json::jesc(msg)))
}

/// 下发认证响应（含 Set-Cookie / 清除 cookie）。
fn send_auth(stream: &mut dyn Io, resp: &AuthResp) {
    let set_cookie = if resp.clear_cookie {
        Some("vp_session=; Path=/; HttpOnly; Max-Age=0")
    } else {
        resp.set_cookie.as_deref()
    };
    let _ = respond_with_headers(stream, &resp.status, &resp.ctype, &resp.body, set_cookie);
}

/// 登录成功后下发会话 cookie；记住我则给 30 天 Max-Age（浏览器关闭仍保留）。
fn sess_cookie(cookie: &str, remember: bool) -> String {
    if remember {
        format!("vp_session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000", cookie)
    } else {
        format!("vp_session={}; Path=/; HttpOnly; SameSite=Lax", cookie)
    }
}

/// 无需会话即可访问的认证端点：登录 / 初始设置 / 退出 / 认证状态。
/// 命中则返回 Some(响应)，否则 None 表示继续走路由守卫。
#[allow(clippy::too_many_arguments)]
fn auth_endpoint(
    state: &State,
    method: &str,
    target: &str,
    body: &[u8],
    cookie: Option<&str>,
    ua: &str,
    client_key: &str,
) -> Option<AuthResp> {
    // 认证状态：前端据此决定显示登录还是初始设置。
    if method == "GET" && target == "/api/auth/state" {
        let needs = state.auth.needs_setup();
        let has = state.auth.has_password();
        return Some(auth_json(format!(
            "{{\"ok\":true,\"enabled\":true,\"needs_setup\":{},\"has_password\":{}}}",
            needs, has
        )));
    }
    // 退出：无需会话，总是可退出（并清 cookie）。
    if method == "POST" && target == "/api/logout" {
        state.auth.logout(cookie, false);
        return Some(AuthResp {
            status: "200 OK".into(),
            ctype: "application/json; charset=utf-8".into(),
            body: b"{\"ok\":true}".to_vec(),
            set_cookie: None,
            clear_cookie: true,
        });
    }
    // 登录。
    if method == "POST" && target == "/api/login" {
        let pw = crate::json::json_field(body, "password").unwrap_or_default();
        let remember = crate::json::json_bool(body, "remember");
        let out = state.auth.login_full(&pw, client_key, ua, remember);
        return match out.kind {
            crate::auth::Login::Ok => {
                let cookie = out.cookie?;
                Some(AuthResp {
                    status: "200 OK".into(),
                    ctype: "application/json; charset=utf-8".into(),
                    body: b"{\"ok\":true}".to_vec(),
                    set_cookie: Some(sess_cookie(&cookie, remember)),
                    clear_cookie: false,
                })
            }
            crate::auth::Login::Bad => Some(auth_err("密码错误")),
            crate::auth::Login::Locked(secs) => {
                Some(auth_err(&format!("登录失败次数过多，请 {} 秒后重试", secs.max(0))))
            }
        };
    }
    // 初始设置：首次访问设置管理员密码。
    if method == "POST" && target == "/api/setup" {
        if !state.auth.needs_setup() {
            return Some(auth_err("已初始化"));
        }
        let pw = crate::json::json_field(body, "password").unwrap_or_default();
        let remember = crate::json::json_bool(body, "remember");
        return match state.auth.setup_full(&pw, ua, remember) {
            Some(cookie) => Some(AuthResp {
                status: "200 OK".into(),
                ctype: "application/json; charset=utf-8".into(),
                body: b"{\"ok\":true}".to_vec(),
                set_cookie: Some(sess_cookie(&cookie, remember)),
                clear_cookie: false,
            }),
            None => Some(auth_err("密码至少 4 位")),
        };
    }
    None
}

/// 已登录的管理端认证操作：改密 / 会话列表 / 强制下线。
/// 命中则返回 Some(响应)，否则 None 交给 /api/* 通用路由。
fn authenticated_admin_endpoint(
    state: &State,
    method: &str,
    target: &str,
    body: &[u8],
    cookie: Option<&str>,
    ua: &str,
) -> Option<AuthResp> {
    // 会话列表。
    if method == "GET" && target == "/api/auth/sessions" {
        return Some(auth_json(state.auth.sessions_json()));
    }
    // 强制下线某个会话。
    if method == "POST" && target == "/api/auth/sessions/revoke" {
        let sid = crate::json::json_field(body, "sid").unwrap_or_default();
        let ok = if sid.is_empty() {
            false
        } else {
            state.auth.revoke(&sid)
        };
        return Some(auth_json(format!("{{\"ok\":{}}}", ok)));
    }
    // 修改密码：成功后保留当前会话、踢掉其它，并下发新 cookie。
    if method == "PUT" && target == "/api/auth/password" {
        let old = crate::json::json_field(body, "old").unwrap_or_default();
        let new = crate::json::json_field(body, "new").unwrap_or_default();
        return match state.auth.change_password(cookie, &old, &new, ua) {
            Some(c) => Some(AuthResp {
                status: "200 OK".into(),
                ctype: "application/json; charset=utf-8".into(),
                body: b"{\"ok\":true}".to_vec(),
                set_cookie: Some(sess_cookie(&c, false)),
                clear_cookie: false,
            }),
            None => Some(auth_err("旧密码错误或新密码太短")),
        };
    }
    None
}