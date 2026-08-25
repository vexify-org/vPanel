//! 路径式反向代理网关（对齐 iotapanel 的 https-front）。
//!
//! 把面板自身端口（HTTP/HTTPS）上的某个路径前缀反向代理到任意本机 TCP 服务，
//! HTTPS 复用内置 TLS 终结。每个连接随请求即时转发，不新开常驻监听线程，
//! 不占用额外进程，技术上不破坏 vPanel 的低内存特性。
//!
//! 提供 MCP 工具与 `/api/proxy/*` 端点管理规则（运行时内存级，重启后由
//! `server.proxies` 配置文件决定）。

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config::{Config, ProxyDef};
use crate::json;

/// 反代规则登记表（随请求读取，常驻内存极小）。
pub struct Proxies {
    list: Mutex<Vec<ProxyDef>>,
}

impl Proxies {
    /// 从配置初始化登记表（首条规则来自 `server.proxies`）。
    pub fn new(cfg: &Config) -> Arc<Proxies> {
        Arc::new(Proxies {
            list: Mutex::new(cfg.server.proxies.clone()),
        })
    }

    /// 新增/更新一条规则：`prefix` 路径前缀 → `target`(host:port)。
    pub fn add(&self, prefix: &str, target: &str) -> (bool, String) {
        let prefix = prefix.trim();
        let target = target.trim();
        if !prefix.starts_with('/') || prefix.len() < 2 || prefix.contains(' ') || !target.contains(':')
        {
            return (false, "prefix 应为 / 开头的路径，target 形如 host:port".to_string());
        }
        let pd = ProxyDef {
            prefix: prefix.to_string(),
            target: target.to_string(),
        };
        let mut l = self.list.lock().unwrap();
        if let Some(pos) = l.iter().position(|p| p.prefix == pd.prefix) {
            l[pos] = pd;
        } else {
            l.push(pd);
        }
        (true, "已添加/更新反代规则".to_string())
    }

    /// 前缀冲突时覆盖，最长前缀优先匹配。
    pub fn match_prefix(&self, uri: &str) -> Option<ProxyDef> {
        if uri.is_empty() {
            return None;
        }
        let l = self.list.lock().unwrap();
        l.iter()
            .filter(|p| uri == p.prefix || uri.starts_with(p.prefix.as_str()))
            .max_by_key(|p| p.prefix.len())
            .cloned()
    }

    pub fn export(&self) -> Vec<ProxyDef> {
        let l = self.list.lock().unwrap();
        l.clone()
    }

    pub fn list_json(&self) -> String {
        let items: Vec<String> = self
            .list
            .lock()
            .unwrap()
            .iter()
            .map(|p| {
                format!(
                    "{{\"prefix\":\"{}\",\"target\":\"{}\"}}",
                    json::jesc(&p.prefix),
                    json::jesc(&p.target)
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    pub fn add_json(&self, prefix: &str, target: &str) -> String {
        let (ok, msg) = self.add(prefix, target);
        format!("{{\"ok\":{},\"msg\":\"{}\"}}", ok, json::jesc(&msg))
    }

    pub fn del(&self, prefix: &str) -> (bool, String) {
        let mut l = self.list.lock().unwrap();
        let before = l.len();
        l.retain(|p| p.prefix != prefix);
        if l.len() == before {
            (false, "未找到该前缀".to_string())
        } else {
            (true, "已删除".to_string())
        }
    }

    pub fn del_json(&self, prefix: &str) -> String {
        let (ok, msg) = self.del(prefix);
        format!("{{\"ok\":{},\"msg\":\"{}\"}}", ok, json::jesc(&msg))
    }
}

/// 执行一次路径反代转发；命中 `def` 时把授权后请求转给目标并回流响应。
/// 返回非空 String 表示由调用方托管写回，空则已直接写回 `client`。
pub fn forward(
    def: &ProxyDef,
    method: &str,
    uri: &str,
    head: &str,
    body: &[u8],
    extra: &[u8],
    client: &mut dyn crate::tls::Io,
    https: bool,
) -> String {
    // 上游路径 = 去掉前缀后的剩余部分（保留 query 与子路径）。
    let rest = uri.get(def.prefix.len()..).unwrap_or("");
    let up_path = {
        let r = rest.trim_start_matches('/');
        if r.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", r)
        }
    };

    let addr: std::net::SocketAddr = match def.target.parse() {
        Ok(a) => a,
        Err(_) => return proxy_error(client, &format!("目标地址无效: {}", def.target)),
    };
    let mut up = match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
        Ok(c) => c,
        Err(e) => return proxy_error(client, &format!("连接目标失败: {}", e)),
    };
    let _ = up.set_nodelay(true);

    let head_lower = head.to_ascii_lowercase();
    let upgrade = head_lower.contains("upgrade: websocket") || header_has(head, "Upgrade", "websocket");

    // 组装转发请求行 + 头。
    let mut req = format!("{} {} HTTP/1.1\r\n", method, up_path);
    for line in head.lines().skip(1) {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let low = t.to_ascii_lowercase();
        if low.starts_with("host:")
            || low.starts_with("connection:")
            || low.starts_with("content-length:")
            || low.starts_with("x-forwarded-proto:")
            || low.starts_with("x-forwarded-host:")
        {
            continue;
        }
        if t.contains(':') && !t.starts_with(' ') {
            req.push_str(t);
            req.push_str("\r\n");
        }
    }
    req.push_str(&format!("Host: {}\r\n", def.target));
    if !upgrade {
        req.push_str("Connection: close\r\n");
    }
    if !body.is_empty() {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    let proto = if https { "https" } else { "http" };
    req.push_str(&format!("X-Forwarded-Proto: {}\r\n", proto));
    req.push_str(&format!("X-Forwarded-Host: {}\r\n", def.target));
    if let Some(ip) = client.peer_ip() {
        req.push_str(&format!("X-Real-IP: {}\r\n", ip));
    }
    req.push_str("\r\n");

    let _ = up.write_all(req.as_bytes());
    if !body.is_empty() {
        let _ = up.write_all(body);
    }
    let _ = up.write_all(extra);
    let _ = up.flush();

    // 读上游响应头（到 \r\n\r\n）。
    let mut resp_buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let _ = up.set_read_timeout(Some(Duration::from_secs(15)));
    loop {
        match up.read(&mut tmp) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                resp_buf.extend_from_slice(&tmp[..n]);
                if find_eoh(&resp_buf).is_some() {
                    break;
                }
            }
        }
    }
    if resp_buf.is_empty() {
        return proxy_error(client, "目标服务返回空响应");
    }

    let head_str = String::from_utf8_lossy(&resp_buf);
    if head_str.starts_with("HTTP/1.1 101") || head_str.starts_with("HTTP/1.0 101") {
        let _ = client.write_all(&resp_buf);
        let _ = client.flush();
        relay(client, up);
        return String::new();
    }

    let _ = client.write_all(&resp_buf);
    loop {
        match up.read(&mut tmp) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if client.write_all(&tmp[..n]).is_err() {
                    break;
                }
            }
        }
    }
    let _ = client.flush();
    String::new()
}

/// 已升级（WebSocket 等 101）连接：双向字节透传。
fn relay(client: &mut dyn crate::tls::Io, up: TcpStream) {
    let mut cli_a = match client.dup() {
        Some(c) => c,
        None => return,
    };
    let mut cli_b = match client.dup() {
        Some(c) => c,
        None => return,
    };
    let mut up_a = match up.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut up_b = up;
    let _ = std::thread::spawn(move || copy_loop(&mut *cli_a, &mut up_a));
    let _ = std::thread::spawn(move || copy_loop(&mut up_b, &mut cli_b));
    std::thread::sleep(Duration::from_millis(10));
}

/// 单向拷贝，直到 EOF/错误。
fn copy_loop<R: Read + ?Sized, W: Write + ?Sized>(from: &mut R, to: &mut W) {
    let mut buf = [0u8; 16384];
    loop {
        match from.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if to.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
        }
    }
}

fn find_eoh(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

fn header_has(head: &str, key: &str, val: &str) -> bool {
    let v = val.to_ascii_lowercase();
    head.lines().skip(1).any(|l| {
        let (k, rv) = match l.split_once(':') {
            Some((k, r)) => (k.trim(), r.trim().to_ascii_lowercase()),
            None => (l.trim(), String::new()),
        };
        k.eq_ignore_ascii_case(key) && rv.contains(&v)
    })
}

fn proxy_error(client: &mut dyn crate::tls::Io, msg: &str) -> String {
    let body = format!("{{\"ok\":false,\"msg\":\"{}\"}}", json::jesc(msg));
    let h = format!(
        "HTTP/1.1 502 Bad Gateway\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = client.write_all(h.as_bytes());
    let _ = client.write_all(body.as_bytes());
    let _ = client.flush();
    String::new()
}