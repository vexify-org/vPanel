//! vPanel — 极简、低常驻内存的 HTTP 面板。
//!
//! 用法:
//!   panel                # 读取 ./panel.yml
//!   panel /path/a.yml    # 读取指定配置

mod api;
mod config;
mod ctl;
mod extra;
mod http;
mod json;
mod lang;
mod mcp;
mod panel;
mod plugins;
mod shop;
mod system;
mod term;
mod ws;

use std::process::ExitCode;

fn main() -> ExitCode {
    // 用法:
    //   panel              # 自动在当前目录查找配置文件（panel.yml / panel.yaml / config.yml / config.yaml）
    //   panel /path/a.yml  # 或显式指定配置路径
    let (cfg, path) = match std::env::args().nth(1) {
        Some(p) => (config::Config::load(&p), Some(p)),
        None => config::Config::auto_find(),
    };

    if let Some(p) = path {
        eprintln!("panel: 已加载配置 {}", p);
    } else {
        eprintln!(
            "panel: 当前目录未找到配置文件，使用默认配置（可创建 panel.yml 覆盖）"
        );
    }

    match http::serve(cfg) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("panel: 无法监听，已退出: {}", e);
            ExitCode::FAILURE
        }
    }
}