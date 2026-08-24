//! Web 终端桥接：把 WebSocket 帧接到一个本地 PTY 上的 Shell。
//!
//! 二条数据流：
//!   WebSocket ——> PTY    在工作线程里读帧并写入 PTY master。
//!   PTY ——> WebSocket    独立线程读取 PTY master 输出，作为二进制帧回发。
//!
//! 伸缩性：每个终端连接占用一个工作线程 + 一个 Shell 子进程；无连接时零开销，
//! 常驻内存不受影响。

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use crate::config::Shell;
use crate::ws::{self, Frame, Ws};

/// 驱动一个终端会话，直到 WebSocket 关闭或 Shell 退出。
pub fn run(mut ws: Ws, shell: &Shell) {
    if !shell.enabled {
        let _ = ws::send_close(&mut *ws.writer);
        return;
    }

    let pty_system = native_pty_system();
    let size = PtySize {
        cols: shell.columns,
        rows: shell.rows,
        pixel_width: 0,
        pixel_height: 0,
    };
    let pair = match pty_system.openpty(size) {
        Ok(p) => p,
        Err(_) => {
            let _ = ws::send_close(&mut *ws.writer);
            return;
        }
    };

    // 由配置拼出命令。
    let mut cmd = CommandBuilder::new(&shell.cmd);
    for a in &shell.args {
        cmd.arg(a);
    }
    let mut child = match pair.slave.spawn_command(cmd) {
        Ok(c) => c,
        Err(_) => {
            let _ = ws::send_close(&mut *ws.writer);
            return;
        }
    };
    drop(pair.slave);

    // master 需要跨线程处理 resize，用 Arc<Mutex> 包裹。
    let master: Arc<Mutex<Box<dyn MasterPty + Send>>> = Arc::new(Mutex::new(pair.master));
    let mut reader = master.lock().unwrap().try_clone_reader().unwrap();
    let mut pty_writer = master.lock().unwrap().take_writer().unwrap();

    // 输出方向：PTY -> WebSocket（独立线程）。
    let ws_out = ws.writer.dup();
    if let Some(mut w_out) = ws_out {
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        let _ = ws::send_close(&mut *w_out);
                        break;
                    }
                    Ok(n) => {
                        if ws::send_binary(&mut *w_out, &buf[..n]).is_err() {
                            let _ = ws::send_close(&mut *w_out);
                            break;
                        }
                    }
                }
            }
        });
    }

    // 输入方向：WebSocket -> PTY（工作线程）。
    'app: loop {
        match ws.read_frame() {
            // 终端按键输入走二进制帧。
            Some(Frame::Binary(data)) | Some(Frame::Text(data)) => {
                // 文本帧如果是 resize 指令（st\tcols\trows）则改尺寸。
                if data.starts_with(b"st\t") {
                    if let Ok(s) = std::str::from_utf8(&data) {
                        let mut it = s.split('\t');
                        if let (Some(_k), Ok(cols), Ok(rows)) =
                            (it.next(), it.next().unwrap_or("").parse::<u16>(), it.next().unwrap_or("").parse::<u16>())
                        {
                            let _ = master.lock().unwrap().resize(PtySize {
                                cols,
                                rows,
                                pixel_width: 0,
                                pixel_height: 0,
                            });
                        }
                    }
                    continue 'app;
                }
                if pty_writer.write_all(&data).is_err() {
                    break 'app;
                }
            }
            Some(Frame::Ping) => {
                let _ = ws::send_pong(&mut *ws.writer);
            }
            Some(Frame::Close) | None => {
                let _ = ws::send_close(&mut *ws.writer);
                break 'app;
            }
            _ => {}
        }
    }

    let _ = child.kill();
}