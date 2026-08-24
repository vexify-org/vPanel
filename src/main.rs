//! vPanel — 极简、低常驻内存的 HTTP 面板。
//!
//! 用法:
//!   panel                # 读取 ./panel.yml
//!   panel /path/a.yml    # 读取指定配置

mod api;
mod auth;
mod backup;
mod config;
mod ctl;
mod db;
mod env;
mod extra;
mod http;
mod iota;
mod json;
mod lang;
mod mcp;
mod monitor;
mod nginx;
mod panel;
mod plugins;
mod shop;
mod ssl;
mod security;
mod system;
mod term;
mod tls;
mod ws;

use std::process::ExitCode;

fn main() -> ExitCode {
    // 用法:
    //   panel                 # 自动在当前目录查找配置文件（panel.yml / panel.yaml / config.yml / config.yaml）
    //   panel /path/a.yml     # 或显式指定配置路径
    //   panel start|stop|restart|log|status|version|help   # CLI 子命令
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("version") | Some("--version") | Some("-v") => {
            println!("vPanel {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some("help") | Some("--help") | Some("-h") => {
            print_usage();
            return ExitCode::SUCCESS;
        }
        Some("status") => {
            print_status();
            return ExitCode::SUCCESS;
        }
        Some("start") => {
            return cli_start();
        }
        Some("stop") => {
            return cli_stop();
        }
        Some("restart") => {
            let _ = cli_stop();
            return cli_start();
        }
        Some("log") => {
            print_log();
            return ExitCode::SUCCESS;
        }
        Some("uninstall") => {
            return cli_uninstall();
        }
        Some("backup") => {
            return cli_backup();
        }
        _ => {}
    }

    let (cfg, path) = match arg {
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

/// 运行状态/运行时文件所在目录。
fn run_dir() -> std::path::PathBuf {
    std::env::var("VPVPANEL_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| ".".into()))
}

fn pid_file() -> std::path::PathBuf {
    run_dir().join("vpanel.pid")
}
fn log_file() -> std::path::PathBuf {
    run_dir().join("vpanel.log")
}

/// 读取 pid 文件，返回当前是否在运行。
fn read_pid() -> Option<u32> {
    let pid: u32 = std::fs::read_to_string(pid_file()).ok()?.trim().parse().ok()?;
    // 进程是否仍存活（Unix 用 kill -0 探测；其它平台仅凭 pid 文件判断）。
    #[cfg(unix)]
    {
        let alive = std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !alive {
            return None;
        }
    }
    Some(pid)
}

fn print_status() {
    let (cfg, _) = config::Config::auto_find();
    println!("vPanel {} (pkg: {})", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_NAME"));
    let scheme = if cfg.server.tls.enabled { "https" } else { "http" };
    println!("listen: {}://{}:{}", scheme, cfg.server.bind, cfg.server.port);
    println!("tls:    {}", if cfg.server.tls.enabled { if cfg.server.tls.cert_file.is_empty() { "enabled (self-signed)" } else { "enabled (existing cert)" } } else { "off" });
    println!("shell:  {} {}", cfg.shell.enabled, cfg.shell.cmd);
    println!("rss_kb: {}", panel::rss_kb());
    println!("auth:   {}", if cfg.security.enabled { "enabled" } else { "disabled" });
    match read_pid() {
        Some(pid) => println!("pid:    {} (running)", pid),
        None => println!("pid:    (not running)"),
    }
}

/// 后台启动：把当前进程作为子进程派发，输出重定向到 vpanel.log，记录 pid。
fn cli_start() -> ExitCode {
    if let Some(pid) = read_pid() {
        eprintln!("panel: 已在运行 (pid {})，如需重启请先 panel stop", pid);
        return ExitCode::FAILURE;
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => {
            eprintln!("panel: 无法定位自身可执行文件");
            return ExitCode::FAILURE;
        }
    };
    // 透传配置文件参数（第二个参数）。
    let config_arg = std::env::args().nth(2);
    let logf = log_file();
    let log = match std::fs::OpenOptions::new().create(true).append(true).open(&logf) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("panel: 打不开日志文件 {}: {}", logf.display(), e);
            return ExitCode::FAILURE;
        }
    };
    let mut cmd = std::process::Command::new(exe);
    if let Some(c) = &config_arg {
        cmd.arg(c);
    }
    cmd.stdout(std::process::Stdio::from(log.try_clone().unwrap()))
        .stderr(std::process::Stdio::from(log));
    match cmd.spawn() {
        Ok(child) => {
            // 父进程把 pid 写入文件后退场；子进程成为孤儿继续运行。
            let _ = std::fs::write(pid_file(), format!("{}", child.id()));
            // 不等待子进程（后台运行）。子进程是非交互的孤儿进程，继续 serve。
            println!("panel: 已在后台启动 (pid {})，日志: {}", child.id(), logf.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("panel: 启动失败: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn cli_stop() -> ExitCode {
    match read_pid() {
        Some(pid) => {
            // 先优雅发 SIGTERM，再兜底。
            let _ = std::process::Command::new("kill")
                .arg(pid.to_string())
                .status();
            std::thread::sleep(std::time::Duration::from_millis(300));
            let still = std::process::Command::new("kill")
                .arg("-0")
                .arg(pid.to_string())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if still {
                let _ = std::process::Command::new("kill")
                    .arg("-9")
                    .arg(pid.to_string())
                    .status();
            }
            let _ = std::fs::remove_file(pid_file());
            println!("panel: 已停止 (pid {})", pid);
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("panel: 未在运行");
            ExitCode::FAILURE
        }
    }
}

fn print_log() {
    let n = std::env::args()
        .nth(3)
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(100);
    let data = match std::fs::read_to_string(log_file()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("panel: 无法读取日志 {}: {}", log_file().display(), e);
            return;
        }
    };
    let lines: Vec<&str> = data.lines().collect();
    for l in lines.iter().skip(lines.len().saturating_sub(n)) {
        println!("{}", l);
    }
}

fn cli_uninstall() -> ExitCode {
    let _ = cli_stop();
    let _ = std::fs::remove_file(log_file());
    println!("panel: 已停止并清理运行时文件（vpanel.pid / vpanel.log）");
    println!("panel: 如需彻底移除二进制，请手动删除自身可执行文件。");
    ExitCode::SUCCESS
}

/// 手动 / 定时执行全量备份（供 crontab 调 `panel backup`）。
fn cli_backup() -> ExitCode {
    let cfg_arg = std::env::args().nth(2);
    let cfg = match cfg_arg {
        Some(p) => config::Config::load(&p),
        None => config::Config::auto_find().0,
    };
    let (ok, msg) = backup::run(&cfg);
    println!("{}", msg);
    exit_code(ok)
}

fn exit_code(ok: bool) -> ExitCode {
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn print_usage() {
    println!(
        "vPanel {} — 极简、低常驻内存的 HTTP 面板\n
用法:
  panel <config.yml>        指定配置文件启动（前台）
  panel                     自动在当前目录查找配置文件（前台）
  panel start [config.yml]  后台启动（输出写入 vpanel.log）
  panel stop                停止后台进程
  panel restart             重启后台进程
  panel log [-n 200]        查看最近日志
  panel status              查看当前状态
  panel backup              手动执行一次全量备份（目录 + 数据库）
  panel uninstall           停止并清理运行时文件
  panel version             显示版本
  panel help                显示本帮助",
        env!("CARGO_PKG_VERSION")
    );
}