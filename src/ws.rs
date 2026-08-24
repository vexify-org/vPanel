//! 手写 WebSocket (RFC 6455) 服务端，仅服务端帧所需的最小实现。
//! 客户端帧必须带掩码；服务端帧不掩码。

use std::io::{self, Write};
use std::time::Duration;

use base64::Engine;
use sha1::{Digest, Sha1};

use crate::tls::Io;

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// WebSocket 帧类型（opcode）。
#[derive(Debug, PartialEq)]
pub enum Frame {
    Text(Vec<u8>),
    Binary(Vec<u8>),
    Close,
    Ping,
    Pong,
}

/// 一个已建立的 WebSocket 连接：reader 负责读客户端帧，writer 负责回写。
/// 读取与写入使用两个独立句柄，便于跨线程（PTY 输出线程只管写）。
pub struct Ws {
    pub reader: Box<dyn Io + Send>,
    pub writer: Box<dyn Io + Send>,
}

impl Ws {
    /// 执行服务端握手（升级到 WebSocket）。
    pub fn accept(conn: Box<dyn Io + Send>, head: &str) -> Option<Ws> {
        let key = head
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("sec-websocket-key:"))?
            .split_once(':')?
            .1
            .trim();
        let accept = ws_accept(key);
        let mut reader = conn;
        // 终端是长连接：设一个较短的读超时，让 TLS 共享句柄能周期让出锁，
        // 否则输出线程会被「等待输入」的读操作饿死。超时被当作「暂无数据」，非断开。
        reader.set_rto(Duration::from_millis(300));
        let writer = reader.dup()?;
        let resp = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            accept
        );
        (&mut *reader).write_all(resp.as_bytes()).ok()?;
        Some(Ws { reader, writer })
    }

    /// 读取下一帧。连接关闭或协议错误返回 None。
    pub fn read_frame(&mut self) -> Option<Frame> {
        let mut hdr = [0u8; 2];
        if read_exact(&mut *self.reader, &mut hdr).is_err() {
            return None;
        }
        let opcode = hdr[0] & 0x0f;
        let masked = hdr[1] & 0x80 != 0;
        let mut len = (hdr[1] & 0x7f) as usize;

        if len == 126 {
            let mut ext = [0u8; 2];
            read_exact(&mut *self.reader, &mut ext).ok()?;
            len = u16::from_be_bytes(ext) as usize;
        } else if len == 127 {
            let mut ext = [0u8; 8];
            read_exact(&mut *self.reader, &mut ext).ok()?;
            len = u64::from_be_bytes(ext) as usize;
        }
        if len > 64 * 1024 * 1024 {
            return None; // 防过度分配
        }

        let mut mask = [0u8; 4];
        if masked && read_exact(&mut *self.reader, &mut mask).is_err() {
            return None;
        }

        let mut payload = vec![0u8; len];
        if len > 0 && read_exact(&mut *self.reader, &mut payload).is_err() {
            return None;
        }
        if masked {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }

        match opcode {
            0x1 => Some(Frame::Text(payload)),
            0x2 => Some(Frame::Binary(payload)),
            0x8 => Some(Frame::Close),
            0x9 => Some(Frame::Ping),
            0xa => Some(Frame::Pong),
            // 0x0 续帧等：这里按数据帧处理，够用即可。
            _ => Some(Frame::Binary(payload)),
        }
    }
}

/// 会话级 7 位长度 < 126 的所能表示的最大长度。
const MAX_LEN7: usize = 125;

/// 向给定写句柄发送一帧（服务端帧，不掩码）。
fn send_frame(w: &mut dyn Io, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(payload.len() + 14);
    buf.push(0x80 | opcode);
    let n = payload.len();
    if n <= MAX_LEN7 {
        buf.push(n as u8);
    } else if n <= 0xffff {
        buf.push(126);
        buf.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        buf.push(127);
        buf.extend_from_slice(&(n as u64).to_be_bytes());
    }
    buf.extend_from_slice(payload);
    w.write_all(&buf)
}

/// 发送二进码数据帧（用于 PTY 输出）。
pub fn send_binary(w: &mut dyn Io, data: &[u8]) -> std::io::Result<()> {
    send_frame(w, 0x2, data)
}

/// 发送 PONG。
pub fn send_pong(w: &mut dyn Io) -> std::io::Result<()> {
    send_frame(w, 0xa, &[])
}

/// 发送关闭帧。
pub fn send_close(w: &mut dyn Io) -> std::io::Result<()> {
    send_frame(w, 0x8, &[])
}

fn ws_accept(key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(GUID.as_bytes());
    let digest = hasher.finalize();
    use base64::engine::general_purpose::STANDARD as B64;
    B64.encode(digest)
}

/// 精确读取 n 字节，否则返回 Err。超时(WouldBlock)/中断视为「暂无数据」，
/// 让出 CPU 稍后重试——避免在 TLS 共享句柄上长时间持有写锁。
fn read_exact(r: &mut dyn Io, buf: &mut [u8]) -> std::io::Result<()> {
    let mut cursor = buf;
    loop {
        match r.read(cursor) {
            Ok(0) => return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "eof")),
            Ok(n) => {
                cursor = &mut cursor[n..];
                if cursor.is_empty() {
                    return Ok(());
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted || e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(e) => return Err(e),
        }
    }
}