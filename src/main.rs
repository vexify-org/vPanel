//! Lumen Panel — 极简、低常驻内存的 HTTP 面板。
//!
//! 用法:
//!   panel                # 读取 ./panel.yml
//!   panel /path/a.yml    # 读取指定配置

mod config;
mod http;
mod panel;

use std::process::ExitCode;

fn main() -> ExitCode {
    let path = std::env::args().nth(1).unwrap_or_else(|| "panel.yml".to_string());
    let cfg = config::Config::load(&path);

    match http::serve(cfg) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("panel: 无法监听，已退出: {}", e);
            ExitCode::FAILURE
        }
    }
}