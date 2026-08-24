//! 内置 HTTPS：rustls 服务器 + 统一连接流抽象。
//!
//! 提供 `Io` 流抽象，普通 TCP 与 TLS 都能被 http/ws 统一读写。WebSocket 终端需要
//! 两个独立句柄（读帧线程 + PTY 输出线程）：普通 TCP 用 `try_clone`（独立 fd），
//! TLS 无法复制连接，退化为「共享互斥句柄」——通过底层 socket 读超时来让出锁，
//! 避免输出线程被读取饿死。
//!
//! TLS 证书支持两种：已有证书（cert_file/key_file）或自动生成一次性自签证书。

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// 统一连接流：普通 TCP 或 TLS（或 TLS 的共享句柄）。
/// 供 http / ws 统一读写，支持复制出一个独立读写句柄。
pub trait Io: Read + Write + Send {
    /// 读超时（尽力而为；TLS 共享句柄用它避免读锁饿死写线程）。
    fn set_rto(&mut self, d: Duration);
    /// 对端 IP（尽力而为）。
    fn peer_ip(&self) -> Option<String>;
    /// 复制出一个独立读写句柄。TCP 复制出独立 fd；TLS 退回共享互斥句柄。
    fn dup(&self) -> Option<Box<dyn Io + Send>>;
}

impl Io for TcpStream {
    fn set_rto(&mut self, d: Duration) {
        let _ = self.set_read_timeout(Some(d));
    }
    fn peer_ip(&self) -> Option<String> {
        self.peer_addr().map(|a| a.ip().to_string()).ok()
    }
    fn dup(&self) -> Option<Box<dyn Io + Send>> {
        self.try_clone().ok().map(|s| Box::new(s) as Box<dyn Io + Send>)
    }
}

/// TLS 连接流：持有 rustls 连接 + 底层 socket（用于对齐读超时与对端 IP）。
struct TlsStream {
    conn: rustls::StreamOwned<rustls::ServerConnection, TcpStream>,
    ip: Option<String>,
}

impl Read for TlsStream {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.conn.read(b)
    }
}
impl Write for TlsStream {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.conn.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.conn.flush()
    }
}
impl Io for TlsStream {
    fn set_rto(&mut self, d: Duration) {
        let _ = self.conn.sock.set_read_timeout(Some(d));
    }
    fn peer_ip(&self) -> Option<String> {
        self.ip.clone()
    }
    fn dup(&self) -> Option<Box<dyn Io + Send>> {
        // TLS 流总是包在 Shared 里，真正的 dup 走 Shared；此处防御性返回 None。
        None
    }
}

/// TLS 的共享互斥句柄：多线程（读帧 + PTY 输出）共用同一 TLS 连接。
struct Shared(Arc<Mutex<Box<dyn Io + Send>>>);

impl Read for Shared {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.0.lock().unwrap().read(b)
    }
}
impl Write for Shared {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}
impl Io for Shared {
    fn set_rto(&mut self, d: Duration) {
        self.0.lock().unwrap().set_rto(d);
    }
    fn peer_ip(&self) -> Option<String> {
        self.0.lock().unwrap().peer_ip()
    }
    fn dup(&self) -> Option<Box<dyn Io + Send>> {
        Some(Box::new(Shared(self.0.clone())))
    }
}

/// 内置 HTTPS 服务器（持有 rustls 配置 + 自签主机名）。
pub struct Server {
    inner: Option<Arc<rustls::ServerConfig>>,
}

impl Server {
    /// 根据配置构建：已启用且给了证书/私钥则加载；否则自动生成自签证书（开箱即用）。
    pub fn build(cfg: &crate::config::Tls) -> io::Result<Server> {
        let inner = if cfg.enabled {
            if !cfg.cert_file.is_empty() && !cfg.key_file.is_empty() {
                Some(Arc::new(make_config(load_certs(&cfg.cert_file)?, load_key(&cfg.key_file)?)?))
            } else {
                let (certs, key) = self_signed(&cfg.host)?;
                Some(Arc::new(make_config(certs, key)?))
            }
        } else {
            None
        };
        Ok(Server { inner })
    }

    pub fn enabled(&self) -> bool {
        self.inner.is_some()
    }

    fn config(&self) -> Option<&Arc<rustls::ServerConfig>> {
        self.inner.as_ref()
    }
}

fn make_config(certs: Vec<CertificateDer<'static>>, key: PrivateKeyDer<'static>) -> io::Result<rustls::ServerConfig> {
    rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad cert/key: {e}")))
}

fn load_certs(path: &str) -> io::Result<Vec<CertificateDer<'static>>> {
    let f = std::fs::File::open(path)?;
    let mut r = io::BufReader::new(f);
    rustls_pemfile::certs(&mut r).collect()
}

fn load_key(path: &str) -> io::Result<PrivateKeyDer<'static>> {
    let f = std::fs::File::open(path)?;
    let mut r = io::BufReader::new(f);
    rustls_pemfile::private_key(&mut r)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "未找到私钥 (private key not found)"))
}

/// 生成一次性自签证书。浏览器会提示证书警告，但能立即可用、免配置。
fn self_signed(host: &str) -> io::Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    use rcgen::{generate_simple_self_signed, CertifiedKey};
    let CertifiedKey { cert, key_pair } = generate_simple_self_signed(vec![host.to_string()])
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("self-sign failed: {e}")))?;
    let cert_der: CertificateDer<'static> = cert.der().clone();
    let pkcs8: rustls::pki_types::PrivatePkcs8KeyDer<'static> =
        key_pair.serialize_der().to_vec().into();
    let key_der = PrivateKeyDer::Pkcs8(pkcs8);
    Ok((vec![cert_der], key_der))
}

/// 把 incoming 连接包装为统一流：TLS 握手或透传。
/// TLS 路径固定 300ms 读超时，让共享互斥句柄能周期性让出锁。
pub fn accept(sock: TcpStream, server: &Server) -> io::Result<Box<dyn Io + Send>> {
    if let (Some(cfg), true) = (server.config(), server.enabled()) {
        let ip = sock.peer_addr().map(|a| a.ip().to_string()).ok();
        let conn = rustls::ServerConnection::new(cfg.clone())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("tls conn: {e}")))?;
        let mut tls = TlsStream { conn: rustls::StreamOwned::new(conn, sock), ip };
        tls.set_rto(Duration::from_millis(300));
        let shared = Shared(Arc::new(Mutex::new(Box::new(tls) as Box<dyn Io + Send>)));
        Ok(Box::new(shared))
    } else {
        Ok(Box::new(sock))
    }
}