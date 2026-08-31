//! IotaPanel 兼容运行时：让 vPanel 直接安装 / 运行 IotaPanel 生态的独立进程插件。
//!
//! 协议与 github.com/plainfate/IotaPanel 完全兼容：
//! - 插件包 = `.tar.gz`，顶层一个目录，内含 `manifest.yaml` + `bin/<command>`；
//! - 启动时分配空闲端口，注入 `PLUGIN_PORT` / `PLUGIN_BIND` / `PLUGIN_NAME` /
//!   `PANEL_HOME` / `IOTAPANEL_VERSION` 环境变量，进程工作组目录切到插件目录；
//! - 网关 `/p/<name>/*` 反向代理到 `bind:port`（见 crate::iota::gateway_proxy）；
//! - 生命周期：按需冷启动、空闲自动退出、keepalive 保活、port-map 跨重启复用、
//!   PID 复用防误杀、崩溃即清理。
//!
//! 目录布局（默认 `<panel_dir>/iota`）：
//!   plugins/<name>/manifest.yaml  插件安装目录
//!   etc/port-map.json             运行中插件端口映射（跨重启复用）
//!   etc/keepalive.json            保活开关
//!   logs/plugins/<name>.log       插件日志（有界轮转）

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

use crate::config::Iota;
use crate::json;

const READY_TIMEOUT: Duration = Duration::from_secs(6);
/// 插件日志单文件字节上限，超限启动时轮转保留一份 .1。
const MAX_LOG_BYTES: u64 = 20 << 20;
/// 远程插件包下载上限（64MB）。
const MAX_PKG: u64 = 64 << 20;

// ---------------------------------------------------------------------------
// manifest.yaml
// ---------------------------------------------------------------------------

/// IotaPanel 插件 manifest.yaml 结构。
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub language: String,
    #[serde(default = "d_bind")]
    pub bind: String,
    /// 相对插件目录的可执行入口，如 bin/terminal。
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub keepalive: bool,
    /// "" = 需面板登录；"none" = 免面板登录（插件自鉴权）。
    #[serde(default)]
    pub auth: String,
    #[serde(default)]
    pub menus: Vec<Menu>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Menu {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub section: String,
}

fn d_bind() -> String {
    "127.0.0.1".to_string()
}

// ---------------------------------------------------------------------------
// 运行时状态
// ---------------------------------------------------------------------------

/// 一个正在运行的插件进程。
struct Rt {
    pid: u32,
    port: u16,
    bind: String,
    /// /proc/<pid>/stat 的启动节拍，防 PID 复用误杀。
    start_tick: u64,
    last_use: Arc<AtomicI64>, // unix 秒
    keepalive: bool,
    child: Arc<Mutex<Option<Child>>>,
}

/// Manager 共享句柄（挂在 HTTP State 上）。
pub struct Manager {
    cfg: Iota,
    runtimes: Mutex<HashMap<String, Rt>>,
    keepalives: Mutex<HashMap<String, bool>>,
    /// 本 Manager 的弱引用（load 后由 Arc 填充），供子进程等待线程回收自身条目。
    self_arc: Mutex<std::sync::Weak<Manager>>,
    /// 空闲回收线程是否已启动（惰性启动，保证首个插件装上时也会拉起）。
    reaper_on: std::sync::atomic::AtomicBool,
}

#[derive(Serialize, Deserialize)]
struct PortMapEntry {
    port: u16,
    pid: u32,
    bind: String,
    started_at: String,
}

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

impl Manager {
    fn plugin_dir(&self, name: &str) -> std::path::PathBuf {
        std::path::Path::new(&self.cfg.home).join("plugins").join(name)
    }
    fn manifest_path(&self, name: &str) -> std::path::PathBuf {
        self.plugin_dir(name).join("manifest.yaml")
    }
    fn port_map_path(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.cfg.home).join("etc").join("port-map.json")
    }
    fn keepalive_path(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.cfg.home).join("etc").join("keepalive.json")
    }
    fn log_path(&self, name: &str) -> std::path::PathBuf {
        std::path::Path::new(&self.cfg.home).join("logs").join("plugins").join(format!("{}.log", name))
    }
    fn home(&self) -> &str {
        &self.cfg.home
    }

    /// 读取已持久化的 keepalive 集合。
    fn load_keepalives(&self) {
        if let Ok(s) = std::fs::read_to_string(self.keepalive_path()) {
            if let Ok(m) = serde_json::from_str::<HashMap<String, bool>>(&s) {
                *self.keepalives.lock().unwrap() = m;
            }
        }
    }
    fn save_keepalives(&self) {
        let m = self.keepalives.lock().unwrap().clone();
        if let Ok(s) = serde_json::to_string(&m) {
            if let Some(parent) = self.keepalive_path().parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(self.keepalive_path(), s);
        }
    }

    /// 构造 Manager 并加载：采纳 port-map 中仍在监听的插件进程，并自愈 keepalive 插件。
    pub fn load(cfg: Iota) -> Arc<Manager> {
        let m = Arc::new(Manager {
            cfg,
            runtimes: Mutex::new(HashMap::new()),
            keepalives: Mutex::new(HashMap::new()),
            self_arc: Mutex::new(std::sync::Weak::new()),
            reaper_on: std::sync::atomic::AtomicBool::new(false),
        });
        *m.self_arc.lock().unwrap() = Arc::downgrade(&m);
        m.load_keepalives();
        m.adopt_running();
        // 仅当确实装了插件时才拉起空闲回收线程；空目录不空转一线程。
        if m.has_any_plugin() {
            m.spawn_idle_reaper();
        }
        m.revive_keepalive();
        m
    }

    /// 插件目录里是否存在任意 manifest（决定是否需要常驻回收线程）。
    fn has_any_plugin(&self) -> bool {
        let dir = std::path::Path::new(&self.cfg.home).join("plugins");
        match std::fs::read_dir(&dir) {
            Ok(rd) => rd.flatten()
                .any(|e| self.manifest_path(&e.file_name().to_string_lossy()).is_file()),
            Err(_) => false,
        }
    }

    /// 扫描已安装插件，把 keepalive 但未运行的冷启动。
    fn revive_keepalive(self: &Arc<Self>) {
        let dir = std::path::Path::new(&self.cfg.home).join("plugins");
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let keep = self.keepalives.lock().unwrap().get(&name).copied().unwrap_or(false);
                if keep && self.manifest_path(&name).is_file() {
                    let _ = self.start(&name);
                }
            }
        }
    }

    fn spawn_idle_reaper(self: &Arc<Self>) {
        // 未启用空闲回收（idle_secs==0）无需常驻回收线程。
        if self.cfg.idle_secs == 0 {
            return;
        }
        // 已有一个回收线程时不再重复起（惰性场景：首个插件装上后由 start() 拉起）。
        if self.reaper_on.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let this = self.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(2));
            let idle = this.cfg.idle_secs;
            if idle == 0 {
                continue;
            }
            let now = now_seconds();
            let mut victims = Vec::new();
            {
                let rt = this.runtimes.lock().unwrap();
                for (name, r) in rt.iter() {
                    if r.keepalive {
                        continue;
                    }
                    if now - r.last_use.load(Ordering::Relaxed) >= idle as i64 {
                        victims.push(name.clone());
                    }
                }
            }
            for v in victims {
                this.stop_quiet(&v);
            }
        });
    }

    /// 采纳 port-map.json 仍在监听的进程（跨核心重启复用，不杀进程）。
    fn adopt_running(&self) {
        let data = match std::fs::read_to_string(self.port_map_path()) {
            Ok(s) => s,
            Err(_) => return,
        };
        let map: HashMap<String, PortMapEntry> = serde_json::from_str(&data).unwrap_or_default();
        for (name, e) in map {
            if e.port == 0 || !is_listening(&e.bind, e.port) {
                let _ = std::fs::remove_dir_all(self.plugin_dir(&name));
                continue;
            }
            let keep = self.keepalives.lock().unwrap().get(&name).copied().unwrap_or(false);
            let tick = proc_start_tick(e.pid);
            let last_use = Arc::new(AtomicI64::new(now_seconds()));
            let rt = Rt {
                pid: e.pid,
                port: e.port,
                bind: e.bind,
                start_tick: tick,
                last_use,
                keepalive: keep,
                child: Arc::new(Mutex::new(None)),
            };
            self.runtimes.lock().unwrap().insert(name, rt);
        }
        self.save_port_map();
    }

    fn save_port_map(&self) {
        let rt = self.runtimes.lock().unwrap();
        let mut out = HashMap::new();
        for (name, r) in rt.iter() {
            out.insert(
                name.clone(),
                PortMapEntry {
                    port: r.port,
                    pid: r.pid,
                    bind: r.bind.clone(),
                    started_at: String::new(),
                },
            );
        }
        if let Ok(s) = serde_json::to_string(&out) {
            if let Some(parent) = self.port_map_path().parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(self.port_map_path(), s);
        }
    }

    /// 插件是否已安装。
    pub fn installed(&self, name: &str) -> bool {
        self.manifest_path(name).is_file()
    }

    /// 读取并校验 manifest。
    pub fn load_manifest(&self, name: &str) -> Option<Manifest> {
        let s = std::fs::read_to_string(self.manifest_path(name)).ok()?;
        let mf: Manifest = serde_yaml::from_str(&s).ok()?;
        if mf.name.is_empty() || mf.command.is_empty() {
            return None;
        }
        Some(mf)
    }

    /// 确保插件在运行，返回 (bind, port)。冷启动等待端口就绪。
    pub fn ensure_running(&self, name: &str) -> Result<(String, u16), String> {
        self.touch(name);
        if let Some(r) = self.runtimes.lock().unwrap().get(name) {
            if port_alive(&r.bind, r.port) {
                return Ok((r.bind.clone(), r.port));
            }
        }
        self.start(name)
    }

    /// 心跳：记录活跃时间（由网关每次访问触发）。
    pub fn touch(&self, name: &str) {
        let rt = self.runtimes.lock().unwrap().get(name).map(|r| r.last_use.clone());
        if let Some(u) = rt {
            u.store(now_seconds(), Ordering::Relaxed);
        }
    }

    /// 冷启动插件进程，等待端口就绪，返回 (bind, port)。
    pub fn start(&self, name: &str) -> Result<(String, u16), String> {
        // 已在运行且端口活着 → 直接复用。
        if let Some(r) = self.runtimes.lock().unwrap().get(name) {
            if port_alive(&r.bind, r.port) {
                return Ok((r.bind.clone(), r.port));
            }
            // 端earkilled，清理残留。
            let pid = r.pid;
            let _ = kill_safe(pid, r.start_tick);
            self.runtimes.lock().unwrap().remove(name);
        }
        let mf = self
            .load_manifest(name)
            .ok_or_else(|| format!("插件未安装或 manifest 无效: {}", name))?;
        let plugin_dir = self.plugin_dir(name);
        let cmd_path = plugin_dir.join(&mf.command);
        if !cmd_path.is_file() {
            return Err(format!("插件入口不存在: {}", mf.command));
        }
        let bind = if mf.bind.is_empty() { "127.0.0.1".to_string() } else { mf.bind.clone() };
        let port = self.alloc_port(&bind)?;

        // 日志文件。
        let log_path = self.log_path(name);
        if let Some(p) = log_path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        rotate_log(&log_path, MAX_LOG_BYTES);
        let logf = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("打不开插件日志: {}", e))?;

        // 构建子进程。
        let keepalive = self.keepalives.lock().unwrap().get(name).copied().unwrap_or(mf.keepalive);
        let child = Command::new(&cmd_path)
            .args(&mf.args)
            .current_dir(&plugin_dir)
            .env("PLUGIN_PORT", port.to_string())
            .env("PLUGIN_BIND", bind.clone())
            .env("PLUGIN_NAME", name)
            .env("PANEL_HOME", self.home())
            .env("IOTAPANEL_VERSION", env!("CARGO_PKG_VERSION"))
            .env("VPANEL_VERSION", env!("CARGO_PKG_VERSION"))
            .stdin(Stdio::null())
            .stdout(Stdio::from(logf.try_clone().map_err(|e| e.to_string())?))
            .stderr(Stdio::from(logf))
            .spawn()
            .map_err(|e| format!("启动插件进程失败: {}", e))?;

        let pid = child.id();
        let start_tick = proc_start_tick(pid);
        let last_use = Arc::new(AtomicI64::new(now_seconds()));
        let rt = Rt {
            pid,
            port,
            bind: bind.clone(),
            start_tick,
            last_use,
            keepalive,
            child: Arc::new(Mutex::new(Some(child))),
        };
        self.runtimes.lock().unwrap().insert(name.to_string(), rt);
        self.save_port_map();

        // 等待端口就绪。
        if !wait_ready(&bind, port) {
            let (pid2, tick) = {
                let rt = self.runtimes.lock().unwrap();
                let r = rt.get(name).expect("just inserted");
                (r.pid, r.start_tick)
            };
            let _ = kill_safe(pid2, tick);
            self.runtimes.lock().unwrap().remove(name);
            self.save_port_map();
            return Err(format!("插件 {} 启动超时（{}）", name, READY_TIMEOUT.as_secs()));
        }

        // 等待线程：回收子进程 + 进程退出时清理运行条目。
        if let Some(arc) = self.self_arc.lock().unwrap().upgrade() {
            arc.spawn_waiter(name.to_string());
        }
        // 首个插件装上后要能空闲回收，确保回收线程在跑。
        if let Some(arc) = self.self_arc.lock().unwrap().upgrade() {
            arc.spawn_idle_reaper();
        }

        eprintln!("panel: iota 插件已启动 {name} ({bind}:{port} pid {pid})");
        Ok((bind, port))
    }

    /// 子进程退出时清理运行条目（回收僵尸，防止网关持续 502 与空闲误杀）。
    fn spawn_waiter(self: &Arc<Self>, name: String) {
        let this = self.clone();
        let child = this.runtimes.lock().unwrap().get(&name).map(|r| r.child.clone());
        if let Some(ch) = child {
            std::thread::spawn(move || {
                let done = {
                    let mut guard = ch.lock().unwrap();
                    match guard.take() {
                        Some(mut c) => {
                            let _ = c.wait();
                            true
                        }
                        None => false,
                    }
                };
                if done {
                    this.runtimes.lock().unwrap().remove(&name);
                    this.save_port_map();
                }
            });
        }
    }

    /// 停止插件进程。
    pub fn stop(&self, name: &str) -> (bool, String) {
        let r = {
            let mut map = self.runtimes.lock().unwrap();
            map.remove(name)
        };
        match r {
            Some(r) => {
                let dead = kill_safe(r.pid, r.start_tick);
                self.save_port_map();
                if dead {
                    (true, format!("插件 {} 已停止", name))
                } else {
                    (false, format!("插件 {} 进程已不存在或 PID 已复用", name))
                }
            }
            None => {
                // 未在运行但仍安装：允许再次 start 前返回状态。
                (false, format!("插件 {} 未在运行", name))
            }
        }
    }

    /// 静默停止（空闲回收用），失败忽略。
    fn stop_quiet(&self, name: &str) {
        let r = self.runtimes.lock().unwrap().remove(name);
        if let Some(r) = r {
            let _ = kill_safe(r.pid, r.start_tick);
            self.save_port_map();
            eprintln!("panel: iota 插件空闲退出 {name}");
        }
    }

    /// 重启插件。
    pub fn restart(&self, name: &str) -> (bool, String) {
        let _ = self.stop(name);
        match self.start(name) {
            Ok((_, port)) => (true, format!("插件 {} 已重启（端口 {port}）", name)),
            Err(e) => (false, e),
        }
    }

    /// 设置保活开关。
    pub fn set_keepalive(&self, name: &str, on: bool) -> (bool, String) {
        if !self.installed(name) {
            return (false, format!("插件 {} 未安装", name));
        }
        self.keepalives.lock().unwrap().insert(name.to_string(), on);
        self.save_keepalives();
        if let Some(r) = self.runtimes.lock().unwrap().get_mut(name) {
            r.keepalive = on;
        }
        let msg = if on {
            format!("插件 {} 已设为保活", name)
        } else {
            format!("插件 {} 已取消保活", name)
        };
        (true, msg)
    }

    /// 在端口池中找一个未被本管理器 / 系统占用的空闲端口。
    fn alloc_port(&self, bind: &str) -> Result<u16, String> {
        for p in self.cfg.port_lo..=self.cfg.port_hi {
            let used = self.runtimes.lock().unwrap().values().any(|r| r.port == p);
            if used {
                continue;
            }
            if !is_listening("127.0.0.1", p) && (bind == "127.0.0.1" || !is_listening(bind, p)) {
                return Ok(p);
            }
        }
        Err("插件端口池已耗尽".to_string())
    }

    // ---- 查询 / 列表 ----

    /// 已安装插件 + 运行状态 + 菜单 -> JSON。
    pub fn list_json(&self) -> String {
        let dir = std::path::Path::new(&self.cfg.home).join("plugins");
        let mut items = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if !e.path().is_dir() {
                    continue;
                }
                let mf = match self.load_manifest(&name) {
                    Some(m) => m,
                    None => continue,
                };
                let (running, port) = {
                    let guard = self.runtimes.lock().unwrap();
                    match guard.get(&name) {
                        Some(r) if port_alive(&r.bind, r.port) => (true, r.port),
                        _ => (false, 0),
                    }
                };
                let keepalive = self.keepalives.lock().unwrap().get(&name).copied().unwrap_or(false);
                let menus: Vec<String> = mf
                    .menus
                    .iter()
                    .map(|m| {
                        format!(
                            "{{\"title\":\"{}\",\"icon\":\"{}\",\"path\":\"{}\",\"section\":\"{}\"}}",
                            json::jesc(&m.title),
                            json::jesc(&m.icon),
                            json::jesc(&m.path),
                            json::jesc(&m.section)
                        )
                    })
                    .collect();
                items.push(format!(
                    "{{\"name\":\"{}\",\"title\":\"{}\",\"version\":\"{}\",\"author\":\"{}\",\"description\":\"{}\",\"language\":\"{}\",\"bind\":\"{}\",\"auth\":\"{}\",\"running\":{},\"port\":{},\"keepalive\":{},\"menus\":[{}]}}",
                    json::jesc(&mf.name),
                    json::jesc(&mf.title),
                    json::jesc(&mf.version),
                    json::jesc(&mf.author),
                    json::jesc(&mf.description),
                    json::jesc(&mf.language),
                    json::jesc(&mf.bind),
                    json::jesc(&mf.auth),
                    running,
                    port,
                    keepalive,
                    menus.join(",")
                ));
            }
        }
        items.sort();
        format!(
            "{{\"ok\":true,\"home\":\"{}\",\"idle_secs\":{},\"list\":[{}]}}",
            json::jesc(&self.cfg.home),
            self.cfg.idle_secs,
            items.join(",")
        )
    }

    /// 单个插件状态 -> JSON。
    pub fn status_json(&self, name: &str) -> String {
        if !self.installed(name) {
            return format!("{{\"ok\":false,\"msg\":\"插件 {} 未安装\"}}", json::jesc(name));
        }
        let (running, port, pid) = {
            let guard = self.runtimes.lock().unwrap();
            match guard.get(name) {
                Some(r) if port_alive(&r.bind, r.port) => (true, r.port, r.pid),
                _ => (false, 0, 0),
            }
        };
        let keepalive = self.keepalives.lock().unwrap().get(name).copied().unwrap_or(false);
        format!(
            "{{\"ok\":true,\"name\":\"{}\",\"running\":{},\"port\":{},\"pid\":{},\"keepalive\":{}}}",
            json::jesc(name),
            running,
            port,
            pid,
            keepalive
        )
    }

    // ---- 安装 / 卸载 ----

    /// 从 https URL 安装插件包（.tar.gz）。强制 https + 必填 SHA256（供应链加固）。对齐 IotaPanel 远程安装。
    pub fn install_url(&self, url: &str, sha256: &str) -> (bool, String) {
        // 供应链加固：只允许 https，且必须显式提供 SHA256 用于校验，杜绝大礼包被替换/降级。
        if !url.starts_with("https://") {
            return (false, "仅支持 https 插件包地址，防止包被中间人替换".to_string());
        }
        if sha256.trim().is_empty() {
            return (false, "必须提供插件包的 SHA256 校验值，拒绝无校验安装".to_string());
        }
        let tmp = std::env::temp_dir().join(format!("vpanel_iota_{}.tgz", std::process::id()));
        let dl = Command::new("curl")
            .args(["-fsSL", "--max-time", "120", "-o"])
            .arg(&tmp)
            .arg(url)
            .status();
        if !matches!(dl, Ok(s) if s.success()) {
            let _ = std::fs::remove_file(&tmp);
            return (false, format!("下载插件包失败: {}", url));
        }
        let size = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
        if size > MAX_PKG {
            let _ = std::fs::remove_file(&tmp);
            return (false, "插件包超过 64MB 上限".to_string());
        }
        // 强校验：sha256 由调用方提供，此处仅比对，不再放行空值。
        if let Ok(o) = Command::new("sha256sum").arg(&tmp).output() {
            let sum = String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_lowercase();
            if o.status.success() && sum == sha256.trim().to_lowercase() {
                let r = self.unpack_install(&tmp);
                let _ = std::fs::remove_file(&tmp);
                return r;
            }
            let _ = std::fs::remove_file(&tmp);
            return (false, "SHA256 校验失败，包可能被篡改或下载不完整".to_string());
        }
        let _ = std::fs::remove_file(&tmp);
        (false, "无法计算 SHA256，安装中止".to_string())
    }

    /// 把 .tar.gz 解压到临时目录，取唯一顶层目录作为插件名，安装到 plugins/<name>。
    fn unpack_install(&self, tgz: &std::path::Path) -> (bool, String) {
        // 解压前先列目录，拒绝含 `..` 段或绝对路径的条目，防 tar-slip 越界写（配合强制 sha256 双保险）。
        if let Ok(o) = Command::new("tar").arg("-tzf").arg(tgz).output() {
            if o.status.success() {
                for name in String::from_utf8_lossy(&o.stdout).lines() {
                    let n = name.trim_end_matches('/').trim_start_matches("./");
                    // 绝对路径或任何 `..` 段都拒绝（tar-slip 越界写）。插件包不应含 `..`。
                    if n.starts_with('/') || n.starts_with("..") || n.split('/').any(|s| s == "..") {
                        return (false, "插件包内包含越界路径，已拒绝安装".to_string());
                    }
                }
            }
        }
        let work = std::env::temp_dir().join(format!("vpanel_iota_x_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::create_dir_all(&work);
        let st = Command::new("tar")
            .arg("-xzf")
            .arg(tgz)
            .arg("-C")
            .arg(&work)
            .status();
        if !matches!(st, Ok(s) if s.success()) {
            let _ = std::fs::remove_dir_all(&work);
            return (false, "解压失败（需要 tar）或包无效".to_string());
        }
        // 找唯一顶层目录。
        let mut tops = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&work) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    tops.push(e.file_name().to_string_lossy().into_owned());
                }
            }
        }
        if tops.len() != 1 {
            let _ = std::fs::remove_dir_all(&work);
            return (false, "插件包应包含且仅包含一个顶层目录".to_string());
        }
        let name = tops[0].clone();
        let src = work.join(&name);
        if !src.join("manifest.yaml").is_file() {
            let _ = std::fs::remove_dir_all(&work);
            return (false, "插件目录缺少 manifest.yaml".to_string());
        }
        let dest = self.plugin_dir(&name);
        let _ = std::fs::remove_dir_all(&dest);
        if std::fs::rename(&src, &dest).is_err() {
            // 跨设备回退拷贝。
            let _ = std::fs::remove_dir_all(&dest);
            if let Some(p) = dest.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            let cp = Command::new("cp").args(["-r"]).arg(&src).arg(&dest).status();
            if !matches!(cp, Ok(s) if s.success()) {
                let _ = std::fs::remove_dir_all(&work);
                return (false, "安装落盘失败".to_string());
            }
        }
        // bin 目录加执行权限（仅 Unix；Windows 按扩展名可执行，无需 chmod）。
        #[cfg(unix)]
        let set_mode = |p: &std::path::Path| -> std::io::Result<()> {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755))
        };
        #[cfg(not(unix))]
        let set_mode = |_p: &std::path::Path| -> std::io::Result<()> { Ok(()) };
        let bin = dest.join("bin");
        if bin.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&bin) {
                for e in rd.flatten() {
                    let _ = set_mode(&e.path());
                }
            }
        }
        let mf = self.load_manifest(&name).unwrap_or_else(|| Manifest {
            name: name.clone(),
            title: name.clone(),
            version: String::new(),
            author: String::new(),
            description: String::new(),
            language: String::new(),
            bind: "127.0.0.1".to_string(),
            command: String::new(),
            args: Vec::new(),
            keepalive: false,
            auth: String::new(),
            menus: Vec::new(),
        });
        // 保活沿用既有设置，否则取 manifest 默认。
        let keep = self.keepalives.lock().unwrap().get(&name).copied().unwrap_or(mf.keepalive);
        self.keepalives.lock().unwrap().insert(name.clone(), keep);
        self.save_keepalives();
        let _ = std::fs::remove_dir_all(&work);
        (true, format!("插件 {} 已安装（v{}，保活 {keep}）", name, mf.version))
    }

    /// 卸载：停止并删除插件目录。
    pub fn uninstall(&self, name: &str) -> (bool, String) {
        let _ = self.stop(name);
        let dest = self.plugin_dir(name);
        if !dest.is_dir() {
            return (false, format!("插件 {} 不存在", name));
        }
        match std::fs::remove_dir_all(&dest) {
            Ok(()) => {
                self.keepalives.lock().unwrap().remove(name);
                self.save_keepalives();
                (true, format!("插件 {} 已卸载", name))
            }
            Err(e) => (false, format!("删除插件目录失败: {}", e)),
        }
    }

    /// 插件日志尾部 JSON。
    pub fn log_tail_json(&self, name: &str, n: usize) -> String {
        let p = self.log_path(name);
        let data = std::fs::read_to_string(&p).unwrap_or_default();
        let lines: Vec<&str> = data.lines().collect();
        let mut it = lines.iter().skip(lines.len().saturating_sub(n.max(1)));
        let mut out = Vec::new();
        for l in it.by_ref() {
            out.push(format!("\"{}\"", json::jesc(l)));
        }
        format!("{{\"ok\":true,\"lines\":[{}]}}", out.join(","))
    }
}

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// ---------------------------------------------------------------------------
// 底层辅助
// ---------------------------------------------------------------------------

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 探测 TCP 端口是否可连接（即被监听）。
fn is_listening(bind: &str, port: u16) -> bool {
    let addr = format!("{}:{}", bind, port);
    match TcpStream::connect_timeout(&addr.parse().ok().unwrap_or("127.0.0.1:0".parse().unwrap()), Duration::from_millis(300)) {
        Ok(_) => true,
        Err(_) => false,
    }
}

fn port_alive(bind: &str, port: u16) -> bool {
    if port == 0 {
        return false;
    }
    is_listening(bind, port)
}

/// 轮询等待端口就绪（最长 READY_TIMEOUT）。
fn wait_ready(bind: &str, port: u16) -> bool {
    let addr = format!("{}:{}", bind, port);
    let deadline = std::time::Instant::now() + READY_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if let Ok(a) = addr.parse() {
            if let Ok(_c) = TcpStream::connect_timeout(&a, Duration::from_millis(200)) {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// 取 /proc/<pid>/stat 启动节拍，防 PID 复用误杀。
fn proc_start_tick(pid: u32) -> u64 {
    let s = std::fs::read_to_string(format!("/proc/{}/stat", pid)).unwrap_or_default();
    // 字段 22（从括号后的相对第 21 个）是 starttime。
    match s.rfind(')') {
        Some(end) => {
            let rest = &s[end + 1..];
            let tok: Vec<&str> = rest.split_whitespace().collect();
            // 括号后第一个字段是 state(字段3)，starttime 是字段22 → 数组下标 19(22-3)。
            tok.get(19).and_then(|x| x.parse().ok()).unwrap_or(0)
        }
        None => 0,
    }
}

/// 安全 kill：校验 PID 启动节拍未变后 SIGKILL。
fn kill_safe(pid: u32, start_tick: u64) -> bool {
    if pid == 0 {
        return false;
    }
    if start_tick > 0 && proc_start_tick(pid) != start_tick {
        return false; // PID 已被复用，绝不能误杀
    }
    let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
    // 等待进程消失。
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(50));
        let alive = Command::new("kill").arg("-0").arg(pid.to_string()).status().map(|s| s.success()).unwrap_or(false);
        if !alive {
            return true;
        }
    }
    true
}

/// 日志轮转：超上限时把文件改名为 .1。
fn rotate_log(path: &std::path::Path, max: u64) {
    if let Ok(md) = std::fs::metadata(path) {
        if md.len() > max {
            let _ = std::fs::rename(path, path.with_extension("log.1"));
        }
    }
}

// ---------------------------------------------------------------------------
// 网关反向代理
// ---------------------------------------------------------------------------

/// 把 `/p/<name>/<path>` 请求反向代理到插件进程端口。
/// 支持 101 Upgrade（WebSocket）双向透传。返回是否已由网关处理。
///
/// `head` 为完整原始请求头文本（含请求行），`body` 为已读取的请求体，
/// `extra` 为请求头缓冲区里 `\r\n\r\n` 之后的多余字节（升级请求可能带早期帧）。
pub fn gateway_proxy(
    cfg: &Iota,
    mgr: &Manager,
    method: &str,
    target: &str,
    head: &str,
    body: &[u8],
    extra: &[u8],
    client: &mut dyn crate::tls::Io,
    https: bool,
) -> String {
    let prefix = &cfg.prefix;
    let rest = target.strip_prefix(prefix).unwrap_or(target);
    let rest = rest.trim_start_matches('/');
    let (name, plugin_path) = match rest.split_once('/') {
        Some((n, p)) => (n, format!("/{}", p)),
        None => (rest, "/".to_string()),
    };
    if name.is_empty() {
        return proxy_error(client, "missing plugin name");
    }
    let (bind, port) = match mgr.ensure_running(name) {
        Ok(x) => x,
        Err(e) => return proxy_error(client, &format!("插件启动失败: {}", e)),
    };
    mgr.touch(name);

    // 建立到插件端口的上游连接。
    let addr = format!("{}:{}", bind, port);
    let mut up = match TcpStream::connect_timeout(&addr.parse().unwrap_or("127.0.0.1:0".parse().unwrap()), Duration::from_secs(3)) {
        Ok(c) => c,
        Err(e) => return proxy_error(client, &format!("连接插件失败: {}", e)),
    };
    let _ = up.set_nodelay(true);

    // 是否升级请求（WebSocket）。
    let head_lower = head.to_ascii_lowercase();
    let upgrade = head_lower.contains("upgrade: websocket") || head_lower.contains("connection: upgrade")
        || header_has(head, "Upgrade", "websocket");

    // 组装转发请求行 + 头。
    let mut req = format!("{} {} HTTP/1.1\r\n", method, plugin_path);
    // 复制原始头，跳过 Host/Connection（自行设定）。
    for line in head.lines().skip(1) {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let low = t.to_ascii_lowercase();
        if low.starts_with("host:")
            || low.starts_with("connection:")
            || low.starts_with("content-length:")
            || low.starts_with("x-forwarded-proto:")
            || low.starts_with("x-forwarded-host:")
        {
            continue;
        }
        // 只保留单行头（简单处理）。
        if t.contains(':') && !t.starts_with(' ') {
            req.push_str(t);
            req.push_str("\r\n");
        }
    }
    req.push_str(&format!("Host: {}:{}\r\n", bind, port));
    if upgrade {
        // 透传 WebSocket 升级头。原始头已含 Upgrade/Sec-WebSocket-*。
    } else {
        req.push_str("Connection: close\r\n");
    }
    // 若原始带 Content-Length 且 body 非空，补上。
    if !body.is_empty() {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    let proto = if https { "https" } else { "http" };
    req.push_str(&format!("X-Forwarded-Proto: {}\r\n", proto));
    req.push_str(&format!("X-Forwarded-Host: {}\r\n", bind));
    req.push_str(&format!("X-Panel-Plugin: {}\r\n", name));
    req.push_str("\r\n");

    // 写请求 + body + 升级前多余字节。
    let _ = up.write_all(req.as_bytes());
    if !body.is_empty() {
        let _ = up.write_all(body);
    }
    let _ = up.write_all(extra);
    let _ = up.flush();

    // 读上游响应头（到 \r\n\r\n）。
    let mut resp_buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let mut head_end = None;
    let _ = up.set_read_timeout(Some(Duration::from_secs(10)));
    while head_end.is_none() {
        match up.read(&mut tmp) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                resp_buf.extend_from_slice(&tmp[..n]);
                if let Some(i) = find_eoh(&resp_buf) {
                    head_end = Some(i);
                    break;
                }
            }
        }
    }

    // 读取失败 / 空响应。
    if resp_buf.is_empty() {
        let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Type: application/json\r\nContent-Length: 32\r\nConnection: close\r\n\r\n{\"error\":\"plugin returned empty\"}");
        let _ = client.flush();
        return String::new();
    }

    // 101 升级 → 双向透传。
    let head_str = String::from_utf8_lossy(&resp_buf);
    if head_str.starts_with("HTTP/1.1 101") || head_str.starts_with("HTTP/1.0 101") {
        let _ = client.write_all(&resp_buf);
        let _ = client.flush();
        relay_ws(client, up);
        return String::new();
    }

    // 普通响应：写出响应头 + 读剩余 body（上游 Connection: close，读到 EOF 即可）。
    let he = head_end.unwrap_or(resp_buf.len());
    // 先写已读到的头部。
    if let Err(_) = client.write_all(&resp_buf[..]) {
        return String::new();
    }
    // 继续读 body 直至 EOF（上游已 close）。
    loop {
        match up.read(&mut tmp) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if client.write_all(&tmp[..n]).is_err() {
                    break;
                }
            }
        }
    }
    let _ = client.flush();
    // Content-Length 没必要手动算；读到 EOF 即完整。he 变量保留以备扩展。
    let _ = he;
    String::new()
}

/// 查找请求头结束位置 `\r\n\r\n`，返回其字节下标。
fn find_eoh(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

/// 头列表里是否存在指定键（值不敏感）。
fn header_has(head: &str, key: &str, val: &str) -> bool {
    let key = key.to_ascii_lowercase();
    for line in head.lines() {
        let t = line.trim();
        if let Some(rest) = t.to_ascii_lowercase().strip_prefix(&(key.clone() + ":")) {
            if rest.trim().eq_ignore_ascii_case(val) {
                return true;
            }
        }
    }
    false
}

/// 网关错误：写 JSON 到客户端。
fn proxy_error(client: &mut dyn crate::tls::Io, msg: &str) -> String {
    let body = format!("{{\"error\":\"{}\"}}", json::jesc(msg));
    let head = format!(
        "HTTP/1.1 502 Bad Gateway\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = client.write_all(head.as_bytes());
    let _ = client.write_all(body.as_bytes());
    let _ = client.flush();
    String::new()
}

/// WebSocket 双向透传：两边各一对独立句柄，两个线程各自读→写。
fn relay_ws(client: &mut dyn crate::tls::Io, up: TcpStream) {
    // 客户端原始句柄复制出两个独立读写入口（TCP 复制 fd；TLS 共享互斥句柄）。
    let mut cli_a = match client.dup() {
        Some(c) => c,
        None => return,
    };
    let mut cli_b = match client.dup() {
        Some(c) => c,
        None => return,
    };
    // 上游 TcpStream 复制成两半。
    let mut up_a = match up.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut up_b = up;
    // cli_a→up_a、up_b→cli_b 两个方向各自独立线程拷贝。
    let _ = std::thread::spawn(move || copy_loop(&mut *cli_a, &mut up_a));
    let _ = std::thread::spawn(move || copy_loop(&mut up_b, &mut cli_b));
    // 当前线程随后即返回；两个后台线程托管连接生命周期。这里等一小下让线程调度。
    std::thread::sleep(Duration::from_millis(10));
}

/// 单向拷贝：从 `from` 读到 `to`，直到 EOF/错误。尽力而为。
fn copy_loop<R: Read + ?Sized, W: Write + ?Sized>(from: &mut R, to: &mut W) {
    let mut buf = [0u8; 16384];
    loop {
        match from.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if to.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_ok() {
        let c = Manager {
            cfg: Iota::default(),
            runtimes: Mutex::new(HashMap::new()),
            keepalives: Mutex::new(HashMap::new()),
            self_arc: Mutex::new(std::sync::Weak::new()),
            reaper_on: std::sync::atomic::AtomicBool::new(false),
        };
        assert!(c.cfg.port_lo > 0);
        assert!(c.cfg.port_hi >= c.cfg.port_lo);
        assert!(c.home().contains("iota"));
    }

    #[test]
    fn manifest_parse() {
        let yaml = "name: hello\ntitle: Hello\ncommand: bin/hello\nmenus:\n  - title: Hello\n    icon: 👋\n    path: /\n    section: tools\n";
        let mf: Manifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(mf.name, "hello");
        assert_eq!(mf.bind, "127.0.0.1"); // 默认
        assert_eq!(mf.menus.len(), 1);
    }

    #[test]
    fn find_eoh_works() {
        assert_eq!(find_eoh(b"GET / HTTP/1.1\r\n\r\n"), Some(18));
        assert_eq!(find_eoh(b"no end"), None);
    }
}