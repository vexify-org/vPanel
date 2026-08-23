//! YAML 配置模型。所有字段均带默认值，允许最小化甚至空配置文件。

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: Server,
    #[serde(default)]
    pub panel: Panel,
    #[serde(default)]
    pub shell: Shell,
}

/// Web 终端（本地 Shell 控制）。
#[derive(Debug, Clone, Deserialize)]
pub struct Shell {
    #[serde(default = "d_enabled")]
    pub enabled: bool,
    #[serde(default = "d_cmd")]
    pub cmd: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "d_columns")]
    pub columns: u16,
    #[serde(default = "d_rows")]
    pub rows: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    #[serde(default = "d_bind")]
    pub bind: String,
    #[serde(default = "d_port")]
    pub port: u16,
    #[serde(default = "d_workers")]
    pub workers: usize,
    #[serde(default = "d_backlog")]
    pub backlog: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Panel {
    #[serde(default = "d_title")]
    pub title: String,
    #[serde(default = "d_subtitle")]
    pub subtitle: String,
    #[serde(default = "d_accent")]
    pub accent: String,
    #[serde(default = "d_theme")]
    pub theme: String,
}

impl Default for Server {
    fn default() -> Self {
        Server {
            bind: d_bind(),
            port: d_port(),
            workers: d_workers(),
            backlog: d_backlog(),
        }
    }
}

impl Default for Panel {
    fn default() -> Self {
        Panel {
            title: d_title(),
            subtitle: d_subtitle(),
            accent: d_accent(),
            theme: d_theme(),
        }
    }
}

impl Default for Shell {
    fn default() -> Self {
        Shell {
            enabled: d_enabled(),
            cmd: d_cmd(),
            args: Vec::new(),
            columns: d_columns(),
            rows: d_rows(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: Server {
                bind: d_bind(),
                port: d_port(),
                workers: d_workers(),
                backlog: d_backlog(),
            },
            panel: Panel {
                title: d_title(),
                subtitle: d_subtitle(),
                accent: d_accent(),
                theme: d_theme(),
            },
            shell: Shell::default(),
        }
    }
}

fn d_bind() -> String {
    "0.0.0.0".to_string()
}
fn d_enabled() -> bool {
    true
}
fn d_cmd() -> String {
    "/bin/sh".to_string()
}
fn d_columns() -> u16 {
    100
}
fn d_rows() -> u16 {
    30
}
fn d_port() -> u16 {
    8080
}
fn d_workers() -> usize {
    4
}
fn d_backlog() -> usize {
    1024
}
fn d_title() -> String {
    "Lumen Panel".to_string()
}
fn d_subtitle() -> String {
    "极简 · 低内存 HTTP 面板".to_string()
}
fn d_accent() -> String {
    "#2563eb".to_string()
}
fn d_theme() -> String {
    "light".to_string()
}

/// 从路径加载 YAML；文件缺失或解析失败时回退到默认配置，
/// 保证进程始终能启动（常驻可用性的第一原则）。
impl Config {
    pub fn load(path: &str) -> Config {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_yaml::from_str(&s).unwrap_or_else(|_| Config::default()),
            Err(_) => Config::default(),
        }
    }
}