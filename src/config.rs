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
    #[serde(default)]
    pub database: Database,
    #[serde(default)]
    pub backup: Backup,
    #[serde(default)]
    pub certs: Certs,
    #[serde(default)]
    pub iota: Iota,
}

/// 登录安全配置。
///
/// `enabled` 默认开启（fail-closed）：未显式配置 `security:` 段的面板也会默认
/// 要求登录。未设置密码时进入初始设置向导，设置完成后所有页面与 API 均需登录。
#[derive(Debug, Clone, Deserialize)]
pub struct Security {
    /// 是否开启登录保护。默认 true。
    #[serde(default = "d_true")]
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

impl Default for Security {
    fn default() -> Self {
        Security {
            enabled: true,
            password: String::new(),
            mcp_token: String::new(),
            max_failures: 5,
            lock_minutes: 5,
            session_hours: 24,
            remember_days: 30,
            single_session: true,
            trust_proxy: false,
        }
    }
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
    /// 内置 HTTPS：TLS 终结。
    #[serde(default)]
    pub tls: Tls,
    /// 路径式反向代理（对齐 iotapanel 的 https-front）：把面板自身端口上的
    /// 某个路径前缀反代到任意本机 TCP 服务；无需额外监听线程，TLS 复用上面 tls。
    #[serde(default)]
    pub proxies: Vec<ProxyDef>,
}

/// 一条反向代理规则：`prefix` 路径前缀 → `target`（host:port），如
/// `{ prefix: "/app", target: "127.0.0.1:8088" }`。
#[derive(Debug, Clone, Deserialize)]
pub struct ProxyDef {
    pub prefix: String,
    pub target: String,
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
            tls: Tls::default(),
            proxies: Vec::new(),
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
                tls: Tls::default(),
                proxies: Vec::new(),
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
            database: Database::default(),
            backup: Backup::default(),
            certs: Certs::default(),
            iota: Iota::default(),
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
    1
}
fn d_backlog() -> usize {
    1024
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

/// 数据库（MySQL/MariaDB）管理配置。
#[derive(Debug, Clone, Deserialize)]
pub struct Database {
    /// 用于管理的数据库账号。
    #[serde(default = "d_db_user")]
    pub user: String,
    /// 该账号密码（留空则用无密码套接字尝试）。
    #[serde(default)]
    pub password: String,
    /// mysql 客户端命令路径。
    #[serde(default = "d_db_bin")]
    pub bin: String,
    /// mysqldump 命令路径。
    #[serde(default = "d_dump_bin")]
    pub dump: String,
    /// 数据库备份目录。
    #[serde(default = "d_db_backup")]
    pub backup_dir: String,
}

impl Default for Database {
    fn default() -> Self {
        Database {
            user: d_db_user(),
            password: String::new(),
            bin: d_db_bin(),
            dump: d_dump_bin(),
            backup_dir: d_db_backup(),
        }
    }
}

/// 备份模块配置。
#[derive(Debug, Clone, Deserialize)]
pub struct Backup {
    /// 备份根目录。
    #[serde(default = "d_broot")]
    pub dir: String,
    /// 每个备份源保留的版本数。
    #[serde(default = "d_bkeep")]
    pub keep: u32,
    /// 定时备份是否启用。
    #[serde(default = "d_bcron")]
    pub cron: String,
}

impl Default for Backup {
    fn default() -> Self {
        Backup {
            dir: d_broot(),
            keep: d_bkeep(),
            cron: d_bcron(),
        }
    }
}

/// 证书（SSL）存储配置。
#[derive(Debug, Clone, Deserialize)]
pub struct Certs {
    /// 证书存放目录（每个站点一个子目录）。
    #[serde(default = "d_certs_dir")]
    pub dir: String,
    /// Let's Encrypt 关联网站（acme.sh 方式，需已安装）。
    #[serde(default)]
    pub le: bool,
}

impl Default for Certs {
    fn default() -> Self {
        Certs {
            dir: d_certs_dir(),
            le: false,
        }
    }
}

fn d_db_user() -> String {
    "root".to_string()
}
fn d_db_bin() -> String {
    "mysql".to_string()
}
fn d_dump_bin() -> String {
    "mysqldump".to_string()
}
fn d_db_backup() -> String {
    format!("{}/db-backup", crate::config::Config::panel_dir())
}
fn d_broot() -> String {
    format!("{}/backup", crate::config::Config::panel_dir())
}
fn d_bkeep() -> u32 {
    5
}
fn d_bcron() -> String {
    "0 3 * * *".to_string()
}
fn d_certs_dir() -> String {
    format!("{}/certs", crate::config::Config::panel_dir())
}

/// IotaPanel 兼容运行时（独立进程插件）配置。
///
/// 对齐 IotaPanel 的插件协议：目录下 `plugins/<name>/manifest.yaml` + `bin/<command>`，
/// 面板分配端口并注入 `PLUGIN_PORT`/`PLUGIN_NAME` 等环境变量，网关 `/p/<name>/*` 反向代理。
#[derive(Debug, Clone, Deserialize)]
pub struct Iota {
    /// 插件根目录（相当于 IotaPanel 的 PANEL_HOME）。默认 `<panel_dir>/iota`。
    #[serde(default = "d_iota_home")]
    pub home: String,
    /// 网关反向代理路由前缀（与 IotaPanel 一致，默认 `/p`）。
    #[serde(default = "d_iota_prefix")]
    pub prefix: String,
    /// 端口池下限。默认 20000。
    #[serde(default = "d_iota_port_lo")]
    pub port_lo: u16,
    /// 端口池上限。默认 21999。
    #[serde(default = "d_iota_port_hi")]
    pub port_hi: u16,
    /// 空闲退出时间（秒）。0 表示不自动退出。默认 300。
    #[serde(default = "d_iota_idle")]
    pub idle_secs: u64,
}

impl Default for Iota {
    fn default() -> Self {
        Iota {
            home: d_iota_home(),
            prefix: d_iota_prefix(),
            port_lo: d_iota_port_lo(),
            port_hi: d_iota_port_hi(),
            idle_secs: d_iota_idle(),
        }
    }
}

fn d_iota_home() -> String {
    format!("{}/iota", crate::config::Config::panel_dir())
}
fn d_iota_prefix() -> String {
    "/p".to_string()
}
fn d_iota_port_lo() -> u16 {
    20000
}
fn d_iota_port_hi() -> u16 {
    21999
}
fn d_iota_idle() -> u64 {
    300
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sane_defaults() {
        let c = Config::default();
        assert_eq!(c.server.port, 8080);
        assert_eq!(c.server.bind, "0.0.0.0");
        assert_eq!(c.server.workers, 1);
        assert_eq!(c.server.backlog, 1024);
        assert!(c.security.enabled);
        assert_eq!(c.plugins.dir, "plugins");
        assert_eq!(c.backup.keep, 5);
        assert!(!c.server.tls.enabled);
        assert_eq!(c.iota.idle_secs, 300);
    }

    #[test]
    fn security_serde_defaults_applied_on_empty_doc() {
        // 未在 YAML 中出现的字段走 serde 默认值（true/5/5/24/30/true）。
        let s: Security = serde_yaml::from_str("").unwrap();
        assert!(s.enabled);
        assert_eq!(s.max_failures, 5);
        assert_eq!(s.lock_minutes, 5);
        assert_eq!(s.session_hours, 24);
        assert_eq!(s.remember_days, 30);
        assert!(s.single_session);
    }

    #[test]
    fn load_missing_file_falls_back_to_default() {
        let c = Config::load("/nonexistent/panel.yml");
        assert_eq!(c.server.port, 8080);
    }

    #[test]
    fn load_parses_override_fields() {
        let dir = std::env::temp_dir()
            .join(format!("vpanel_cfg_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("panel.yml");
        std::fs::write(&path,
            "server:\n  port: 9090\nshell:\n  enabled: false\nsecurity:\n  max_failures: 9\n")
            .unwrap();
        let c = Config::load(path.to_str().unwrap());
        assert_eq!(c.server.port, 9090);
        assert!(!c.shell.enabled);
        assert_eq!(c.security.max_failures, 9);
        // 未覆盖字段仍用默认
        assert_eq!(c.server.workers, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_invalid_yaml_falls_back_to_default() {
        let dir = std::env::temp_dir()
            .join(format!("vpanel_cfg_bad_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.yml");
        std::fs::write(&path, "server: [unclosed").unwrap();
        let c = Config::load(path.to_str().unwrap());
        assert_eq!(c.server.port, 8080);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn auto_find_returns_none_when_no_config() {
        // auto_find 依赖当前工作目录；在没有配置文件的临时目录应回退默认且路径为 None。
        let old = std::env::current_dir().unwrap();
        let dir = std::env::temp_dir().join(format!("vpanel_af_{}", std::process::id()));
        std::fs::create_dir_all(&dir)
            .and_then(|_| std::env::set_current_dir(&dir))
            .unwrap();
        let (c, p) = Config::auto_find();
        assert_eq!(c.server.port, 8080);
        assert!(p.is_none());
        let _ = std::env::set_current_dir(&old);
        let _ = std::fs::remove_dir_all(&dir);
    }
}