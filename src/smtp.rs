//! 极简 SMTP 客户端：发送纯文本邮件。零额外运行进程。
//!
//! 三种传输模式：`none`（明文）、`ssl`（连接即 SMTPS/TLS）、`starttls`（先明文
//! 握手再升级 TLS）。TLS 封装复用 rustls，仅当编译时启用 `tls` 特性才可用；
//! 精简构建下 SSL/STARTTLS 会返回明确错误提示，明文仍可发内网邮件。
//!
//! 采用逐字节读响应行而不是 BufReader，避免缓存多余字节损坏 STARTTLS 升级后的流。

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// 传输层抽象：明文或（tls 特性下的）TLS 加密流，由 `Option` 承载以便 STARTTLS 热替换。
enum Conn {
    Plain(TcpStream),
    #[cfg(feature = "tls")]
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.read(buf),
            #[cfg(feature = "tls")]
            Conn::Tls(s) => s.read(buf),
        }
    }
}
impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.write(buf),
            #[cfg(feature = "tls")]
            Conn::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Conn::Plain(s) => s.flush(),
            #[cfg(feature = "tls")]
            Conn::Tls(s) => s.flush(),
        }
    }
}

// ---- 根证书缓存：仅 tls 特性 ----
#[cfg(feature = "tls")]
fn roots_store() -> rustls::RootCertStore {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    roots
}

/// 用 rustls 把明文 TCP 包装为加密流（SSL 直连 / STARTTLS 升级共用）。
#[cfg(feature = "tls")]
fn tls_wrap(host: &str, tcp: TcpStream) -> Result<Conn, String> {
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots_store())
        .with_no_client_auth();
    let server = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|_| "SMTP 主机名不合法，无法建立 TLS".to_string())?;
    let conn = rustls::ClientConnection::new(std::sync::Arc::new(config), server)
        .map_err(|e| format!("TLS 握手失败: {}", e))?;
    Ok(Conn::Tls(Box::new(rustls::StreamOwned::new(conn, tcp))))
}

/// 无 tls 特性时的提示占位。
#[cfg(not(feature = "tls"))]
fn tls_wrap(_host: &str, _tcp: TcpStream) -> Result<Conn, String> {
    Err("当前构建未启用加密 SMTP（需 --features tls）；SSMTP/STARTTLS 不可用".into())
}

/// 会话：持有传输流 + 一次性写缓冲，提供 SMTP 命令交换。
struct Session {
    conn: Option<Conn>,
    out: Vec<u8>,
}

impl Session {
    fn conn(&mut self) -> &mut Conn {
        self.conn.as_mut().expect("conn 已取走")
    }
    /// 发一行命令。
    fn cmd(&mut self, line: &str) -> Result<(), String> {
        self.out.extend_from_slice(line.as_bytes());
        self.out.extend_from_slice(b"\r\n");
        Ok(())
    }
    /// 刷新并读取一行响应，要求以给定状态码开头。
    fn expect(&mut self, want: &str) -> Result<(), String> {
        self.flush()?;
        let line = self.read_line()?;
        if line.starts_with(want) {
            Ok(())
        } else {
            Err(format!("SMTP 响应异常: {}", line.trim()))
        }
    }
    /// 刷新并读取多行响应（如 EHLO 的 250-… 续行），直到出现独立状态行。
    fn expect_multi(&mut self, want: &str) -> Result<(), String> {
        self.flush()?;
        loop {
            let line = self.read_line()?;
            if line.starts_with(want) {
                // "250-text" 为续行；"250 text"（第 4 字符为空格）或单行 250 为结束。
                if line.as_bytes().get(3) == Some(&b' ') {
                    return Ok(());
                }
            } else if line.starts_with("550") || line.starts_with("554") {
                return Err(format!("SMTP 拒绝: {}", line.trim()));
            }
        }
    }
    /// 逐字节读一行（不含换行）。
    fn read_line(&mut self) -> Result<String, String> {
        let mut buf = Vec::with_capacity(48);
        let mut b = [0u8; 1];
        loop {
            match self.conn().read(&mut b) {
                Ok(0) => return Err("SMTP 连接被关闭".into()),
                Ok(_) => {
                    if b[0] == b'\n' {
                        break;
                    }
                    buf.push(b[0]);
                }
                Err(e) => return Err(format!("读取 SMTP 响应失败: {}", e)),
            }
        }
        Ok(String::from_utf8_lossy(&buf).trim_end_matches('\r').to_string())
    }
    fn flush(&mut self) -> Result<(), String> {
        if self.out.is_empty() {
            return Ok(());
        }
        let data = std::mem::take(&mut self.out);
        self.conn().write_all(&data).map_err(|e| e.to_string())?;
        self.conn().flush().map_err(|e| e.to_string())?;
        Ok(())
    }
    /// STARTTLS：取回明文流并热替换为加密流（tls 特性下生效）。
    fn upgrade(&mut self, host: &str) -> Result<(), String> {
        let plain = match self.conn.take() {
            Some(Conn::Plain(p)) => p,
            _ => return Err("流已加密或状态异常".into()),
        };
        let wrapped = tls_wrap(host, plain)?;
        self.conn = Some(wrapped);
        Ok(())
    }
}

/// 发送一封邮件。`to` 可为多个收件人。返回 `Ok(())` 表示服务器已接受。
pub fn send(
    host: &str,
    port: u16,
    mode: &str,
    user: &str,
    pass: &str,
    from: &str,
    to: &[String],
    subject: &str,
    body: &str,
) -> Result<(), String> {
    if host.is_empty() || from.is_empty() || to.is_empty() {
        return Err("SMTP 服务器、发件人、收件人不能为空".into());
    }

    // ---- 连接 ----
    let addr = format!("{}:{}", host, port);
    let tcp = TcpStream::connect(&addr)
        .map_err(|e| format!("无法连接 SMTP {} ({})", addr, e))?;
    tcp.set_read_timeout(Some(Duration::from_secs(20))).ok();
    tcp.set_write_timeout(Some(Duration::from_secs(20))).ok();

    // SSL：先包 TLS 再读欢迎语；none/starttls 先明文。
    let conn = match mode {
        "ssl" => tls_wrap(host, tcp)?,
        _ => Conn::Plain(tcp),
    };
    let mut s = Session { conn: Some(conn), out: Vec::with_capacity(256) };
    s.expect("220")?; // 服务就绪

    // ---- 打招呼 ----
    let ehlo = ehlo_name(host);
    s.cmd(&format!("EHLO {}", ehlo))?;
    s.expect_multi("250")?;

    // ---- STARTTLS 升级 ----
    if mode == "starttls" {
        s.cmd("STARTTLS")?;
        s.expect("220")?;
        s.upgrade(host)?;
        s.cmd(&format!("EHLO {}", ehlo))?;
        s.expect_multi("250")?;
    }

    // ---- 认证 ----
    if !user.is_empty() {
        s.cmd("AUTH LOGIN")?;
        s.expect("334")?;
        s.cmd(&b64(user))?;
        s.expect("334")?;
        s.cmd(&b64(pass))?;
        s.expect("235")?;
    }

    // ---- 发件 / 收件 ----
    s.cmd(&format!("MAIL FROM:<{}>", from))?;
    s.expect("250")?;
    for rcpt in to {
        if rcpt.trim().is_empty() {
            continue;
        }
        s.cmd(&format!("RCPT TO:<{}>", rcpt.trim()))?;
        s.expect("250")?;
    }

    // ---- 内容 ----
    s.cmd("DATA")?;
    s.expect("354")?;
    s.out.extend_from_slice(&compose_mail(from, to, subject, body));
    s.flush()?;
    // dot-stuffing：正文中行首的 '.' 要再加一个 '.'。
    s.out.extend_from_slice(b"\r\n");
    s.flush()?;
    s.out.extend_from_slice(b".\r\n");
    s.flush()?;
    s.expect("250")?;

    // ---- 结束 ----
    let _ = s.cmd("QUIT");
    let _ = s.flush();
    Ok(())
}

fn compose_mail(from: &str, to: &[String], subject: &str, body: &str) -> Vec<u8> {
    let date = now_rfc822();
    let tos = to.join(", ");
    let mut m = String::with_capacity(body.len() + 256);
    m.push_str("From: ");
    m.push_str(from);
    m.push_str("\r\nTo: ");
    m.push_str(&tos);
    m.push_str("\r\nSubject: ");
    // RFC 2047 编码非 ASCII 主题（UTF-8 base64）。
    if subject.is_ascii() {
        m.push_str(subject);
    } else {
        m.push_str("=?UTF-8?B?");
        m.push_str(&b64(subject));
        m.push_str("?=");
    }
    m.push_str("\r\nDate: ");
    m.push_str(&date);
    m.push_str("\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n");
    // dot-stuffing：行首 '.' 加 '.'；并以 CRLF 换行。
    let mut first = true;
    for line in body.split('\n') {
        if !first {
            m.push_str("\r\n");
        }
        first = false;
        let t = line.trim_end_matches('\r');
        if t.starts_with('.') {
            m.push('.');
        }
        m.push_str(t);
    }
    m.into_bytes()
}

/// base64 编码（用于 AUTH LOGIN 与 RFC2047 主题）。
fn b64(s: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
}

/// 当前时间 —— RFC 822 / RFC 1123 格式（GMT）。
fn now_rfc822() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let (y, mo, d, hh, mm, ss) = civil_from_days(days as i64, secs % 86400);
    const MON: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    const DOW: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let wd = ((days.wrapping_add(4)) % 7) as usize; // 1970-01-01 是周四
    format!(
        "{}, {:02} {} {} {:02}:{:02}:{:02} +0000",
        DOW[wd], d, MON[(mo - 1) as usize], y, hh, mm, ss
    )
}

/// 把 unix 秒格式化为 "YYYY-MM-DD HH:MM:SS"（UTC），供告警正文使用。
pub(crate) fn format_secs(secs: u64) -> String {
    let days = secs / 86400;
    let (y, mo, d, hh, mm, ss) = civil_from_days(days as i64, secs % 86400);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, hh, mm, ss)
}

/// 秒 -> 公历日期（Howard Hinnant 的 civil_from_days）。
fn civil_from_days(z: i64, secs: u64) -> (i64, i64, i64, u64, u64, u64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, secs % 86400 / 3600, secs % 3600 / 60, secs % 60)
}

fn ehlo_name(host: &str) -> String {
    let s: String = host
        .chars()
        .take(60)
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '.')
        .collect();
    if s.is_empty() { "vpanel".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b64_works() {
        assert_eq!(b64("user"), "dXNlcg==");
        assert_eq!(b64("pass"), "cGFzcw==");
    }

    #[test]
    fn date_sane() {
        let d = now_rfc822();
        assert!(d.contains(" 20"), "date should contain a space + 20xx year: {}", d);
    }

    #[test]
    fn ehlo_filters() {
        assert_eq!(ehlo_name("[1.2.3.4]"), "1.2.3.4");
        assert_eq!(ehlo_name(""), "vpanel");
    }
}