//! 软件商店：实时从远程仓库拉取应用清单 + 一键下载/安装。
//!
//! 设计要点：
//! - 清单由一个 GitHub 仓库（默认 vexify-org/vp-store 的 apps.yml）维护，
//!   面板「一键爬取」= 走加速前缀实时拉取该清单并缓存，改软件不用改面板代码。
//! - 安装脚本按需 `bash -c` 一次性执行，与面板常驻内存解耦。
//! - 全局下载统一走加速前缀（默认 https://g.z321.cc.cd/，配置可改）。

use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::Config;

/// 单个应用条目（对应 apps.yml 里的每一项）。
#[derive(Debug, Clone, Deserialize)]
pub struct App {
    pub id: String,
    pub name: String,
    pub desc: String,
    /// 安装脚本模板，`{accel}` 会被替换为加速前缀。
    #[serde(default)]
    pub script: String,
}

/// apps.yml 的顶层结构。
#[derive(Debug, Deserialize)]
struct AppsFile {
    #[serde(default)]
    apps: Vec<App>,
}

/// 共享的商店状态：拉取结果缓存。
pub struct Shop {
    /// 最近一次成功拉取的清单。
    cached: Mutex<StoreCache>,
}

struct StoreCache {
    apps: Vec<App>,
    accel: String,
    last: Option<Instant>,
    /// 清单来源："remote"（远程仓库）或 "builtin"（内置）。
    mode: &'static str,
}

impl Shop {
    pub fn new() -> Arc<Shop> {
        Arc::new(Shop {
            cached: Mutex::new(StoreCache {
                apps: Vec::new(),
                accel: String::new(),
                last: None,
                mode: "builtin",
            }),
        })
    }

    /// 拉取最近清单；带 60s 缓存。远程拉取成功用远程；失败回退内置清单。
    pub fn fetch(&self, cfg: &Config) -> (bool, String) {
        {
            let c = self.cached.lock().unwrap();
            if let Some(t) = c.last {
                if t.elapsed() < Duration::from_secs(60) && !c.apps.is_empty() {
                    return (true, String::new());
                }
            }
        }
        let accel = accel_of(cfg);
        let url = make_list_url(&cfg, &accel);
        // 尝试远程拉取清单。清单可达 1MB+，慢网络下 4s 超时过短，放宽到 15s。
        if let Ok(o) = std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", "15", &url])
            .output()
        {
            if o.status.success() {
                if let Ok(f) = serde_yaml::from_slice::<AppsFile>(&o.stdout) {
                    let mut c = self.cached.lock().unwrap();
                    c.apps = f.apps;
                    c.accel = accel;
                    c.mode = "remote";
                    c.last = Some(Instant::now());
                    return (true, String::new());
                }
            }
        }
        // 回退：用内置清单。
        let mut c = self.cached.lock().unwrap();
        if c.apps.is_empty() {
            c.apps = default_apps();
            c.accel = accel;
            c.mode = "builtin";
        }
        c.last = Some(Instant::now());
        let note = if c.mode == "builtin" {
            "远程仓库暂不可用，当前使用内置清单".to_string()
        } else {
            String::new()
        };
        (true, note)
    }

    /// 商店列表 -> JSON。返回 ok、来源模式、可选提示、应用列表。
    pub fn list_json(&self, cfg: &Config) -> String {
        let (_ok, msg) = self.fetch(cfg);
        let c = self.cached.lock().unwrap();
        let mut out = format!(
            "{{\"ok\":true,\"mode\":\"{}\",\"accel\":\"{}\",\"len\":{}",
            c.mode,
            crate::json::jesc(c.accel.trim_end_matches('/')),
            c.apps.len()
        );
        if !msg.is_empty() {
            out.push_str(&format!(",\"msg\":\"{}\"", crate::json::jesc(&msg)));
        }
        out.push_str(",\"list\":[");
        for (i, a) in c.apps.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"id\":\"{}\",\"name\":\"{}\",\"desc\":\"{}\"}}",
                crate::json::jesc(&a.id),
                crate::json::jesc(&a.name),
                crate::json::jesc(&a.desc),
            ));
        }
        out.push_str("]}");
        out
    }

    /// 一键安装某软件。返回 (成功, 输出末尾文本)。
    pub fn install(&self, app_id: &str, cfg: &Config) -> (bool, String, /*exists*/ bool) {
        if let (false, msg) = self.fetch(cfg) {
            return (false, msg, false);
        }
        let (script, accel) = {
            let c = self.cached.lock().unwrap();
            match c.apps.iter().find(|a| a.id == app_id) {
                Some(a) => (a.script.clone(), c.accel.clone()),
                None => return (false, format!("未知软件: {}", app_id), false),
            }
        };
        let script = script.replace("{accel}", &accel);
        let out = std::process::Command::new("bash")
            .arg("-c")
            .arg(&script)
            .output();
        match out {
            Ok(o) => {
                let tail = chunk_tail(&o.stdout, &o.stderr, 600);
                (o.status.success(), tail, true)
            }
            Err(e) => (false, e.to_string(), true),
        }
    }
}

/// 组装清单的远程 URL：加速前缀 + raw.githubusercontent 的 apps.yml。
fn make_list_url(cfg: &Config, accel: &str) -> String {
    let repo = cfg.download.store.trim().trim_end_matches('/');
    // repo 形如 "vexify-org/vp-store@main" 或 "vexify-org/vp-store"
    let (path, branch) = match repo.split_once('@') {
        Some((p, b)) => (p, b),
        None => (repo, "main"),
    };
    format!(
        "{}https://raw.githubusercontent.com/{}/refs/heads/{}/apps.yml",
        accel, path, branch
    )
}

/// 取加速前缀，保证以 `/` 结尾。
fn accel_of(cfg: &Config) -> String {
    let a = cfg.download.accel.trim().trim_end_matches('/');
    if a.is_empty() {
        DEFAULT_ACCEL.to_string()
    } else {
        format!("{}/", a)
    }
}

/// 取 stdout+stderr 末尾文本（限制长度）。
fn chunk_tail(stdout: &[u8], stderr: &[u8], max: usize) -> String {
    let mut s = String::from_utf8_lossy(stdout).into_owned();
    if !s.is_empty() {
        s.push('\n');
    }
    s.push_str(&String::from_utf8_lossy(stderr));
    if s.chars().count() > max {
        s = s.chars().skip(s.chars().count() - max).collect();
        s = "[…截断] ".to_string() + &s;
    }
    s.trim().to_string()
}

pub const DEFAULT_ACCEL: &str = "https://g.z321.cc.cd/";
pub const DEFAULT_STORE: &str = "vexify-org/vp-store@main";

/// 内置兜底清单：优先解析仓库自带完整 `apps.yml`（编译期嵌入，约 5000 应用），
/// 保证远程仓库（vp-store）不可用时面板依然拥有完整的软件商店；解析失败才回退极致精简的常用 8 项。
fn default_apps() -> Vec<App> {
    // 编译期嵌入仓库 `.vp-store/apps.yml`，随二进制分发，离线也可用完整商店。
    if let Ok(f) = serde_yaml::from_str::<AppsFile>(include_str!("../.vp-store/apps.yml")) {
        return f.apps;
    }
    vec![
        App { id: "nginx".into(), name: "Nginx".into(), desc: "高性能 Web / 反向代理服务器".into(), script: "set -e\napt-get update -qq\napt-get install -y -qq nginx\n".into() },
        App { id: "php".into(), name: "PHP (FPM)".into(), desc: "PHP 脚本运行时 + 常用扩展".into(), script: "set -e\napt-get update -qq\nDEBIAN_FRONTEND=noninteractive apt-get install -y -qq php-fpm php-cli php-mysql php-curl php-gd php-intl php-mbstring php-xml php-zip php-redis\nsystemctl enable php-fpm 2>/dev/null || true\n".into() },
        App { id: "docker".into(), name: "Docker".into(), desc: "容器运行时（官方脚本，走加速）".into(), script: "set -e\ncurl -fsSL \"{accel}https://get.docker.com\" | sh\n".into() },
        App { id: "redis".into(), name: "Redis".into(), desc: "内存键值数据库".into(), script: "set -e\napt-get update -qq && apt-get install -y -qq redis-server\n".into() },
        App { id: "mysql".into(), name: "MySQL".into(), desc: "关系型数据库".into(), script: "set -e\napt-get update -qq\nDEBIAN_FRONTEND=noninteractive apt-get install -y -qq mysql-server\n".into() },
        App { id: "go".into(), name: "Go".into(), desc: "Go 语言工具链（官方 tarball，走加速）".into(), script: "set -e\ncurl -fsSL \"{accel}https://go.dev/dl/go1.23.2.linux-amd64.tar.gz\" | tar -C /usr/local -xz\necho 'export PATH=$PATH:/usr/local/go/bin' >> /etc/profile\n".into() },
        App { id: "node".into(), name: "Node.js".into(), desc: "Node.js 运行时（NodeSource 脚本，走加速）".into(), script: "set -e\ncurl -fsSL \"{accel}https://deb.nodesource.com/setup_lts.x\" | bash -\napt-get install -y -qq nodejs\n".into() },
        App { id: "fail2ban".into(), name: "Fail2ban".into(), desc: "暴力破解防护".into(), script: "set -e\napt-get update -qq && apt-get install -y -qq fail2ban\nsystemctl enable --now fail2ban\n".into() },
    ]
}