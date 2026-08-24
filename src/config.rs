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
    #[serde(default)]
    pub download: Download,
    #[serde(default)]
    pub plugins: Plugins,
    #[serde(default)]
    pub security: Security,
    /// 文件管理根目录（可选）。非空时所有文件读写/删除/下载均被限制在此目录内，
    /// 防止以面板权限（常为 root）越权操作系统任意文件。留空则不限制（向后兼容）。
    #[serde(default)]
    pub fs_root: String,
}

/// 文件管理根目录（全局）。`None` 表示不限制。在 `http::serve` 启动时初始化一次。
static FS_ROOT: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();

/// 从配置（与环境变量 `VPANEL_FS_ROOT`）解析并设置文件根目录。
pub fn init_fs_root(cfg: &Config) {
    let raw = std::env::var("VPANEL_FS_ROOT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            let s = cfg.fs_root.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        });
    let root = raw.map(|s| std::path::PathBuf::from(s.trim().trim_end_matches('/').to_string()));
    let _ = FS_ROOT.set(root);
}

/// 返回当前文件管理根目录（规范化后）。`None` 表示不限制。
pub fn fs_root() -> Option<std::path::PathBuf> {
    FS_ROOT
        .get()
        .cloned()
        .flatten()
        .and_then(|p| p.canonicalize().ok().or(Some(p)))
}

/// 登录安全配置。
///
/// `enabled` 默认关闭以保持向后兼容；开启后未设置密码会进入初始设置向导，
/// 设置完成后所有页面与 API 均需登录。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Security {
    /// 是否开启登录保护。默认 false。
    #[serde(default)]
    pub enabled: bool,
    /// 管理员密码（明文，仅用于首次设置；设置成功后存入哈希文件，可留空走向导）。
    #[serde(default)]
    pub password: String,
    /// MCP 端点的独立 Bearer 令牌（可选）。留空则 MCP 需走面板登录会话。
    #[serde(default)]
    pub mcp_token: String,
    /// 连续失败多少次后锁定。
    #[serde(default = "d_max_failures")]
    pub max_failures: u32,
    /// 锁定时长（分钟）。
    #[serde(default = "d_lock_minutes")]
    pub lock_minutes: u32,
    /// 会话有效期（小时）。
    #[serde(default = "d_session_hours")]
    pub session_hours: u32,
    /// 「记住我」有效期（天），登录勾选记住我时使用。
    #[serde(default = "d_remember_days")]
    pub remember_days: u32,
    /// 单账号单会话：新登录自动踢掉旧会话。默认 true（与 IotaPanel 一致）。
    #[serde(default = "d_true")]
    pub single_session: bool,
    /// 面板位于受信 HTTPS 反向代理之后时置真，用于识别 HTTPS 并开放代理头部。
    #[serde(default)]
    pub trust_proxy: bool,
}

fn d_remember_days() -> u32 {
    30
}
fn d_true() -> bool {
    true
}

fn d_max_failures() -> u32 {
    5
}
fn d_lock_minutes() -> u32 {
    5
}
fn d_session_hours() -> u32 {
    24
}

/// 插件系统配置。
#[derive(Debug, Clone, Deserialize)]
pub struct Plugins {
    /// 插件目录（相对运行目录）。默认 `plugins`。
    #[serde(default = "d_plugin_dir")]
    pub dir: String,
}

impl Default for Plugins {
    fn default() -> Self {
        Plugins { dir: d_plugin_dir() }
    }
}

/// 当前时区偏移（秒），默认 +8（Asia/Shanghai），可用环境变量 TZ_OFFSET 覆盖。
static TZ: std::sync::OnceLock<i64> = std::sync::OnceLock::new();

pub fn tz() -> &'static i64 {
    TZ.get_or_init(|| {
        std::env::var("TZ_OFFSET")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(28800)
    })
}

fn d_plugin_dir() -> String {
    "plugins".to_string()
}

/// 下载 / 加速配置。
#[derive(Debug, Clone, Deserialize)]
pub struct Download {
    /// 全局下载加速前缀，用于软件商店下载。空则用默认代理。
    #[serde(default = "d_accel")]
    pub accel: String,
    /// 软件商店清单仓库，形如 "owner/repo@branch"。空则用默认。
    #[serde(default = "d_store")]
    pub store: String,
    /// 插件商店里 `kind: docker` 的包解压目标目录。默认 `/docker`。
    #[serde(default = "d_docker_dir")]
    pub docker_dir: String,
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
    /// 请求体最大字节数（防护：避免超大 body 撑爆内存）。默认 16MB。
    #[serde(default = "d_max_body")]
    pub max_body: usize,
    /// 内置 HTTPS：TLS 终结。
    #[serde(default)]
    pub tls: Tls,
}

/// 内置 HTTPS 配置（对齐 iotapanel 的 https-front）。
#[derive(Debug, Clone, Deserialize)]
pub struct Tls {
    /// 是否开启内置 TLS。默认 false。
    #[serde(default)]
    pub enabled: bool,
    /// 已有证书 PEM 路径（cert）与私钥 PEM 路径（key）。两者都填则使用已有证书，
    /// 否则自动生成一次性自签证书（开箱即用，浏览器会提示证书警告）。
    #[serde(default)]
    pub cert_file: String,
    #[serde(default)]
    pub key_file: String,
    /// 自签证书的通用名 / SAN（用于识别）。默认 "vpanel"。
    #[serde(default = "d_tls_host")]
    pub host: String,
}

impl Default for Tls {
    fn default() -> Self {
        Tls {
            enabled: false,
            cert_file: String::new(),
            key_file: String::new(),
            host: d_tls_host(),
        }
    }
}

fn d_tls_host() -> String {
    "vpanel.local".to_string()
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
            max_body: d_max_body(),
            tls: Tls::default(),
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

impl Default for Download {
    fn default() -> Self {
        Download {
            accel: d_accel(),
            store: d_store(),
            docker_dir: d_docker_dir(),
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
                max_body: d_max_body(),
                tls: Tls::default(),
            },
            panel: Panel {
                title: d_title(),
                subtitle: d_subtitle(),
                accent: d_accent(),
                theme: d_theme(),
            },
            shell: Shell::default(),
            download: Download::default(),
            plugins: Plugins::default(),
            security: Security::default(),
            fs_root: String::new(),
        }
    }
}

fn d_accel() -> String {
    crate::shop::DEFAULT_ACCEL.to_string()
}

fn d_store() -> String {
    crate::shop::DEFAULT_STORE.to_string()
}

fn d_docker_dir() -> String {
    "/docker".to_string()
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
fn d_max_body() -> usize {
    16 * 1024 * 1024
}
fn d_title() -> String {
    "vPanel".to_string()
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

/// 当前工作目录下会自动尝试的配置文件名（按优先级）。
const CANDIDATES: &[&str] = &["panel.yml", "panel.yaml", "config.yml", "config.yaml"];

/// 从路径加载 YAML；文件缺失或解析失败时回退到默认配置，
/// 保证进程始终能启动（常驻可用性的第一原则）。
impl Config {
    /// 面板数据目录：默认当前工作目录，可用环境变量 VPVPANEL_DIR 覆盖。
    pub fn panel_dir() -> String {
        std::env::var("VPVPANEL_DIR")
            .unwrap_or_else(|_| match std::env::var("PWD") {
                Ok(p) if !p.is_empty() => p,
                _ => ".".to_string(),
            })
    }

    pub fn load(path: &str) -> Config {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_yaml::from_str(&s).unwrap_or_else(|_| Config::default()),
            Err(_) => Config::default(),
        }
    }

    /// 自动在「当前工作目录」查找配置文件，返回 (配置, 实际命中的文件名)。
    /// 找不到任何候选时返回默认配置与 None，进程仍能正常启动。
    pub fn auto_find() -> (Config, Option<String>) {
        for name in CANDIDATES {
            if std::path::Path::new(name).is_file() {
                if let Ok(s) = std::fs::read_to_string(name) {
                    if let Ok(cfg) = serde_yaml::from_str(&s) {
                        return (cfg, Some((*name).to_string()));
                    }
                }
            }
        }
        (Config::default(), None)
    }
}