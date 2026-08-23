//! 插件系统：极简 DSL（YAML）描述 + 微脚本运行时，注入面板 API / MCP。
//!
//! - 插件目录（默认 `plugins/`）下的每个 `*.yml` 即一个插件。
//! - 插件可声明 `tools`（注入到 `/api/plugin/<name>/<tool>` 与 MCP）、
//!   `tasks`（面板内自带定时器周期执行，不依赖 crontab）、
//!   `hooks`（挂到面板事件）。
//! - 脚本使用自研微语言（见 [`crate::lang`]），内置 `cmd`/`fetch`/`ret`/`log` 等。
//! - 脚本只在执行时新建解释器，跑完即释放，常驻内存保持有界。

use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::lang::{Builtin, Interp};
use crate::json;

/// 面板内置的 20 个事件钩子（事件表）。插件可按 `event` 注册脚本。
///
/// 目前实际触发点：`on_init`(启动)、`on_shutdown`(退出)、`on_tick`(每秒)、
/// `on_http_request`(每个 HTTP 请求)。其余事件供插件声明，后续面板动作处触发。
#[allow(dead_code)]
pub const HOOKS: [&str; 20] = [
    "on_init",
    "on_shutdown",
    "on_tick",
    "on_http_request",
    "on_snapshot",
    "on_process_list",
    "on_service_start",
    "on_service_stop",
    "on_service_restart",
    "on_firewall_allow",
    "on_firewall_del",
    "on_task_add",
    "on_task_del",
    "on_login",
    "on_logout",
    "on_shop_install",
    "on_disk_low",
    "on_cpu_high",
    "on_mem_high",
    "on_cron",
];

/// 插件清单文件顶层结构。
#[derive(Debug, Clone, Deserialize)]
struct PluginFile {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    desc: String,
    #[serde(default)]
    tools: Vec<Tool>,
    #[serde(default)]
    tasks: Vec<Task>,
    #[serde(default)]
    hooks: Vec<Hook>,
}

/// 一个工具入参：用于在前端渲染表单、在 MCP 生成 inputSchema。
#[derive(Debug, Clone, Deserialize)]
pub struct Param {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "d_param_type")]
    pub r#type: String, // string | number | bool | select
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub desc: String,
    /// 可选候选项（type=select 时使用），逗号分隔。
    #[serde(default)]
    pub options: String,
}

fn d_param_type() -> String {
    "string".to_string()
}

/// 一个工具：暴露成面板 API 的一键能力。
#[derive(Debug, Clone, Deserialize)]
pub struct Tool {
    pub id: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub script: String,
    /// 入参表单：前端据此渲染表单，MCP 据此生成 inputSchema。
    #[serde(default)]
    pub params: Vec<Param>,
}

/// 一个插件内的定时任务。`every` 单位为秒。
#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    pub id: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default = "d_5")]
    pub every: u64,
    #[serde(default)]
    pub script: String,
}

/// 挂到特定事件上的脚本。
#[derive(Debug, Clone, Deserialize)]
pub struct Hook {
    pub event: String,
    #[serde(default)]
    pub script: String,
}

fn d_5() -> u64 {
    5
}

/// 加载后的插件（运行时视图）。
#[derive(Debug, Clone)]
pub struct Plugin {
    pub name: String,
    pub version: String,
    pub desc: String,
    pub tools: Vec<Tool>,
    pub tasks: Vec<Task>,
    pub hooks: Vec<Hook>,
}

impl From<PluginFile> for Plugin {
    fn from(f: PluginFile) -> Self {
        Plugin {
            name: f.name,
            version: f.version,
            desc: f.desc,
            tools: f.tools,
            tasks: f.tasks,
            hooks: f.hooks,
        }
    }
}

/// 共享的插件注册表 + 最近日志（有界）+ 持久化 KV + 参数字段。
pub struct Plugins {
    list: Mutex<Vec<Plugin>>,
    logs: Mutex<VecDeque<String>>,
    task_fire: Mutex<std::collections::HashMap<String, std::time::Instant>>,
    kv: Mutex<std::collections::HashMap<String, String>>,
    /// 被禁用的插件名集合（持久化）。
    disabled: Mutex<std::collections::HashSet<String>>,
    /// 插件商店缓存（远程清单 + 最近拉取时刻）。
    store: Mutex<StoreCache>,
}

/// 插件商店缓存：远程拉取的可用插件清单。
struct StoreCache {
    items: Vec<StoreItem>,
    last: Option<std::time::Instant>,
    mode: &'static str,
}

/// 插件商店里的一个条目（对应仓库 plugins.yml）。
#[derive(Debug, Clone, Deserialize)]
struct StoreItem {
    id: String,
    name: String,
    #[serde(default)]
    desc: String,
    /// 仓库内插件清单相对路径，如 `plugins/demo.yml`。
    file: String,
}

/// plugins.yml 顶层结构。
#[derive(Debug, Deserialize)]
struct StoreFile {
    #[serde(default)]
    plugins: Vec<StoreItem>,
}

/// 命令 / KV / 入参的系统执行能力的实现（每次执行新建，持有一个借用）。
struct CmdBuiltin<'a> {
    args: &'a std::collections::HashMap<String, String>,
    kv: &'a std::collections::HashMap<String, String>,
}

impl<'a> CmdBuiltin<'a> {
    fn new(args: &'a std::collections::HashMap<String, String>, kv: &'a std::collections::HashMap<String, String>) -> Self {
        CmdBuiltin { args, kv }
    }
}

impl<'a> Builtin for CmdBuiltin<'a> {
    fn cmd(&self, shell: &str) -> String {
        match std::process::Command::new("/bin/sh").arg("-c").arg(shell).output() {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            Ok(o) => {
                let e = String::from_utf8_lossy(&o.stderr);
                format!("(exit {}) {}", o.status.code().unwrap_or(-1), e.trim())
            }
            Err(e) => format!("(cmd err) {}", e),
        }
    }
    fn fetch(&self, url: &str, timeout: u64) -> String {
        let t = timeout.to_string();
        match std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", &t, url])
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            Err(e) => format!("(fetch err) {}", e),
        }
    }
    fn kv_get(&self, key: &str) -> Option<String> {
        self.kv.get(key).cloned()
    }
    fn kv_set(&self, _key: &str, _val: &str) -> bool {
        // 由宿主在解释器 take_kv_writes() 后写回；此处为只读视图。
        true
    }
    fn arg(&self, name: &str) -> Option<String> {
        self.args.get(name).cloned()
    }
    fn has_arg(&self, name: &str) -> bool {
        self.args.contains_key(name)
    }
}

impl Plugins {
    pub fn new() -> Arc<Plugins> {
        Arc::new(Plugins {
            list: Mutex::new(Vec::new()),
            logs: Mutex::new(VecDeque::new()),
            task_fire: Mutex::new(std::collections::HashMap::new()),
            kv: Mutex::new(std::collections::HashMap::new()),
            disabled: Mutex::new(std::collections::HashSet::new()),
            store: Mutex::new(StoreCache {
                items: Vec::new(),
                last: None,
                mode: "builtin",
            }),
        })
    }
    fn load_kv(&self) {
        if let Ok(s) = std::fs::read_to_string(kv_path()) {
            if let Ok(h) = serde_json::from_str::<std::collections::HashMap<String, String>>(&s) {
                *self.kv.lock().unwrap() = h;
            }
        }
    }
    fn save_kv(&self) {
        let s = {
            let m = self.kv.lock().unwrap();
            serde_json::to_string(&*m).unwrap_or_default()
        };
        let _ = std::fs::write(kv_path(), s);
    }

    /// 插件快照（clone，避免调用方持有锁）。
    pub fn snapshot(&self) -> Vec<Plugin> {
        self.list.lock().unwrap().clone()
    }

    /// 从配置指定的目录加载全部 `*.yml` 插件。
    pub fn load(self: &Arc<Self>, cfg: &Config) {
        let dir = cfg.plugins.dir.clone();
        self.load_disabled();
        self.reload_dir(&dir);
        self.load_kv();
        self.run_hooks("on_init");
        self.spawn_scheduler();
    }

    /// 重新扫描插件目录（热重载：更新 / 卸载后即时生效，旧内存自然释放）。
    /// 不重复触发 on_init 调度线程。
    pub fn reload(&self, dir: &str) {
        self.reload_dir(dir);
        self.push_log("panel", "插件目录已热重载".to_string());
    }

    /// 扫描目录并替换内存中的插件列表。
    fn reload_dir(&self, dir: &str) {
        let read = match std::fs::read_dir(dir) {
            Ok(r) => r,
            Err(_) => {
                eprintln!("panel: 插件目录 {} 不存在，已忽略", dir);
                self.list.lock().unwrap().clear();
                return;
            }
        };
        let mut loaded: Vec<Plugin> = Vec::new();
        for ent in read.flatten() {
            let path = ent.path();
            if path.extension().map(|e| e == "yml" || e == "yaml").unwrap_or(false) {
                if let Ok(s) = std::fs::read_to_string(&path) {
                    if let Ok(f) = serde_yaml::from_str::<PluginFile>(&s) {
                        let name = if f.name.is_empty() {
                            path.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default()
                        } else {
                            f.name.clone()
                        };
                        let p: Plugin = f.into();
                        let p = Plugin { name, ..p };
                        eprintln!("panel: 加载插件 {}", p.name);
                        loaded.push(p);
                    } else {
                        eprintln!("panel: 插件 {} 解析失败，已跳过", path.display());
                    }
                }
            }
        }
        *self.list.lock().unwrap() = loaded;
    }

    // ---- 启用 / 禁用 ----

    /// 某插件当前是否启用。
    pub fn is_enabled(&self, name: &str) -> bool {
        !self.disabled.lock().unwrap().contains(name)
    }

    /// 设置启用 / 禁用并持久化。
    pub fn set_enabled(&self, name: &str, on: bool) -> (bool, String) {
        if !self.list.lock().unwrap().iter().any(|p| p.name == name) {
            return (false, format!("未找到插件 {}", name));
        }
        {
            let mut d = self.disabled.lock().unwrap();
            if on {
                d.remove(name);
            } else {
                d.insert(name.to_string());
            }
        }
        self.save_disabled();
        let msg = if on {
            format!("插件 {} 已启用", name)
        } else {
            format!("插件 {} 已禁用", name)
        };
        self.push_log("panel", msg.clone());
        (true, msg)
    }

    fn load_disabled(&self) {
        let path = disabled_path();
        if let Ok(s) = std::fs::read_to_string(path) {
            if let Ok(v) = serde_json::from_str::<Vec<String>>(&s) {
                let mut d = self.disabled.lock().unwrap();
                d.clear();
                d.extend(v);
            }
        }
    }
    fn save_disabled(&self) {
        let v: Vec<String> = self.disabled.lock().unwrap().iter().cloned().collect();
        let _ = std::fs::write(disabled_path(), serde_json::to_string(&v).unwrap_or_else(|_| "[]".to_string()));
    }

    /// 启动自带定时线程：每秒检查各插件的 task，到期即执行。
    /// 记录上次触发时刻到 `task_fire`（单调时钟），保证周期稳定、不随 tick 漂移。
    fn spawn_scheduler(self: &Arc<Self>) {
        let this = self.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let list = this.list.lock().unwrap().clone();
            let now = std::time::Instant::now();
            for p in &list {
                if !this.is_enabled(&p.name) {
                    continue;
                }
                for t in &p.tasks {
                    let key = format!("{}/{}", p.name, t.id);
                    if this.due(&key, t.every.max(1), now) {
                        let noargs = std::collections::HashMap::new();
                        this.exec_script(&p.name, &t.script, &format!("[task {}]", t.id), noargs);
                    }
                }
            }
            this.run_hooks("on_tick");
        });
    }

    /// 判断周期任务是否到期；首次立即执行一次。
    fn due(&self, key: &str, every: u64, now: std::time::Instant) -> bool {
        let mut m = self.task_fire.lock().unwrap();
        let last = m.entry(key.to_string()).or_insert_with(|| now);
        if now.duration_since(*last).as_secs() >= every {
            *last = now;
            true
        } else {
            false
        }
    }

    /// 执行一段脚本（shared 路径），记录日志到有界环。
    /// `args` 为工具入参。KV 写入会持久化到 kv_path()。
    pub fn exec_script(&self, tag: &str, script: &str, label: &str, args: std::collections::HashMap<String, String>) -> (bool, String) {
        // 取当前 KV 快照，与解释器同作用域一起释放；解释器产出的 KV 写回后再落盘。
        let (out_value, logs, writes) = {
            let kv = self.kv.lock().unwrap();
            let builtin = CmdBuiltin::new(&args, &kv);
            let mut it = Interp::with_prefix(tag.to_string(), &builtin);
            match it.run(script) {
                Ok(o) => (o.value, o.logs, it.take_kv_writes()),
                Err(e) => {
                    self.push_log(tag, format!("脚本错误: {}", e));
                    return (false, e);
                }
            }
        };
        // 持久化 KV 写入。
        if !writes.is_empty() {
            for (k, v) in writes {
                self.kv.lock().unwrap().insert(k, v);
            }
            self.save_kv();
        }
        for l in logs.iter() {
            self.push_log(tag, l.clone());
        }
        let msg = if out_value.is_empty() {
            format!("{} 已执行", label)
        } else {
            match label.is_empty() {
                true => out_value,
                false => format!("{}: {}", label, out_value),
            }
        };
        (true, msg)
    }

    fn push_log(&self, tag: &str, line: String) {
        let mut l = self.logs.lock().unwrap();
        l.push_back(format!("[{}, {}]", chrono_now(), tag) + &" " + &line);
        while l.len() > 200 {
            l.pop_front();
        }
    }

    /// 触发指定事件钩子。
    pub fn run_hooks(&self, event: &str) {
        let list = self.list.lock().unwrap();
        for p in list.iter() {
            if !self.is_enabled(&p.name) {
                continue;
            }
            for h in &p.hooks {
                if h.event == event {
                    let noargs = std::collections::HashMap::new();
                    let _ = self.exec_script(&p.name, &h.script, &format!("[hook {}]", event), noargs);
                }
            }
        }
    }

    /// 调用某个插件的某个工具，可带参。
    pub fn call_tool(&self, plugin: &str, tool: &str, args: std::collections::HashMap<String, String>) -> (bool, String) {
        let script = {
            let list = self.list.lock().unwrap();
            list.iter()
                .filter(|p| p.name == plugin && self.is_enabled(&p.name))
                .flat_map(|p| p.tools.iter())
                .find(|t| t.id == tool)
                .map(|t| t.script.clone())
        };
        match script {
            Some(s) => self.exec_script(plugin, &s, &format!("[tool {}]", tool), args),
            None => (false, format!("未找到插件 {}/{}（或已被禁用）", plugin, tool)),
        }
    }

    // ---- 插件商店：在线安装 / 更新 ----

    /// 拉取插件商店清单，带 60s 缓存。远程失败回退内置（空）清单。
    pub fn store_fetch(&self, cfg: &Config) {
        {
            let c = self.store.lock().unwrap();
            if let Some(t) = c.last {
                if t.elapsed() < std::time::Duration::from_secs(60) {
                    return;
                }
            }
        }
        let accel = accel_of(cfg);
        let url = store_list_url(cfg, &accel);
        if let Ok(o) = std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", "4", &url])
            .output()
        {
            if o.status.success() {
                if let Ok(f) = serde_yaml::from_slice::<StoreFile>(&o.stdout) {
                    let mut c = self.store.lock().unwrap();
                    c.items = f.plugins;
                    c.mode = "remote";
                    c.last = Some(std::time::Instant::now());
                    return;
                }
            }
        }
        let mut c = self.store.lock().unwrap();
        if c.items.is_empty() {
            c.items = default_store();
            c.mode = "builtin";
        }
        c.last = Some(std::time::Instant::now());
    }

    /// 插件商店列表 -> JSON。
    pub fn store_list_json(&self, cfg: &Config) -> String {
        self.store_fetch(cfg);
        let c = self.store.lock().unwrap();
        let arr: Vec<String> = c
            .items
            .iter()
            .map(|i| {
                format!(
                    "{{\"id\":\"{}\",\"name\":\"{}\",\"desc\":\"{}\",\"file\":\"{}\"}}",
                    json::jesc(&i.id),
                    json::jesc(&i.name),
                    json::jesc(&i.desc),
                    json::jesc(&i.file)
                )
            })
            .collect();
        format!(
            "{{\"ok\":true,\"mode\":\"{}\",\"list\":[{}]}}",
            c.mode,
            arr.join(",")
        )
    }

    /// 从商店下载插件 yml 到插件目录并热重载。相当于「安装」或「更新/升级」。
    pub fn store_install(&self, id: &str, cfg: &Config) -> (bool, String) {
        self.store_fetch(cfg);
        let (file, accel) = {
            let c = self.store.lock().unwrap();
            match c.items.iter().find(|i| i.id == id) {
                Some(i) => (i.file.clone(), accel_of(cfg)),
                None => return (false, format!("商店里没有插件 {}", id)),
            }
        };
        let url = raw_url(cfg, &file, &accel);
        let dl = std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", "20", &url])
            .output();
        let content = match dl {
            Ok(o) if o.status.success() => o.stdout,
            Ok(o) => return (false, format!("下载失败: {}", String::from_utf8_lossy(&o.stderr).trim())),
            Err(e) => return (false, format!("下载失败: {}", e)),
        };
        // 校验下载的确实是合法插件 yml。
        if serde_yaml::from_slice::<PluginFile>(&content).is_err() {
            return (false, "下载内容不是合法的插件清单".to_string());
        }
        let file = std::path::Path::new(&file)
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("plugin.yml");
        let dest = format!("{}/{}", cfg.plugins.dir.trim_end_matches('/'), file);
        if let Err(e) = std::fs::write(&dest, &content) {
            return (false, format!("写文件失败: {}", e));
        }
        self.reload(&cfg.plugins.dir);
        self.push_log("panel", format!("已安装/更新插件 {} -> {}", id, dest));
        (true, format!("插件 {} 已安装到 {}", id, dest))
    }

    /// 卸载插件：删除其 yml 文件并热重载，内存随即释放。
    pub fn store_uninstall(&self, plugin: &str, cfg: &Config) -> (bool, String) {
        let dir = &cfg.plugins.dir;
        let removed = self.remove_plugin_file(plugin, dir);
        self.reload(dir);
        if removed {
            self.push_log("panel", format!("已卸载插件 {}", plugin));
            (true, format!("插件 {} 已卸载", plugin))
        } else {
            (false, format!("未找到插件 {} 的清单文件", plugin))
        }
    }

    /// 删除指定插件名对应的文件（按 yml 内 name 匹配）。
    fn remove_plugin_file(&self, plugin: &str, dir: &str) -> bool {
        let items = match std::fs::read_dir(dir) {
            Ok(r) => r.flatten().collect::<Vec<_>>(),
            Err(_) => return false,
        };
        for ent in items {
            let path = ent.path();
            if !path.extension().map(|e| e == "yml" || e == "yaml").unwrap_or(false) {
                continue;
            }
            if let Ok(s) = std::fs::read_to_string(&path) {
                if let Ok(f) = serde_yaml::from_str::<PluginFile>(&s) {
                    let name = if f.name.is_empty() {
                        path.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default()
                    } else {
                        f.name.clone()
                    };
                    if name == plugin {
                        let _ = std::fs::remove_file(&path);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 插件 KV 文件路径（放当前运行目录 .vp 隐藏文件）。
    pub fn kv_list_json(&self) -> String {
        let m = self.kv.lock().unwrap();
        let items: Vec<String> = m
            .iter()
            .map(|(k, v)| format!("{{\"k\":\"{}\",\"v\":\"{}\"}}", json::jesc(k), json::jesc(v)))
            .collect();
        format!("{{\"ok\":true,\"kv\":[{}]}}", items.join(","))
    }

    /// 插件列表 JSON（供前端 /api/plugins）。
    pub fn list_json(&self) -> String {
        let list = self.list.lock().unwrap();
        let logs: Vec<String> = self.logs.lock().unwrap().iter().rev().take(30).cloned().collect();
        let arr: Vec<String> = list
            .iter()
            .map(|p| {
                let enabled = self.is_enabled(&p.name);
                let tools: Vec<String> = p
                    .tools
                    .iter()
                    .map(|t| {
                        let params: Vec<String> = t
                            .params
                            .iter()
                            .map(|pp| {
                                format!(
                                    "{{\"id\":\"{}\",\"name\":\"{}\",\"type\":\"{}\",\"required\":{},\"default\":\"{}\",\"desc\":\"{}\",\"options\":\"{}\"}}",
                                    json::jesc(&pp.id),
                                    json::jesc(&pp.name),
                                    json::jesc(&pp.r#type),
                                    pp.required,
                                    json::jesc(&pp.default),
                                    json::jesc(&pp.desc),
                                    json::jesc(&pp.options)
                                )
                            })
                            .collect();
                        format!(
                            "{{\"id\":\"{}\",\"desc\":\"{}\",\"params\":[{}]}}",
                            json::jesc(&t.id),
                            json::jesc(&t.desc),
                            params.join(",")
                        )
                    })
                    .collect();
                let tasks: Vec<String> = p
                    .tasks
                    .iter()
                    .map(|t| {
                        format!(
                            "{{\"id\":\"{}\",\"every\":{},\"desc\":\"{}\"}}",
                            json::jesc(&t.id),
                            t.every,
                            json::jesc(&t.desc)
                        )
                    })
                    .collect();
                let hooks: Vec<String> = p
                    .hooks
                    .iter()
                    .map(|h| format!("{{\"event\":\"{}\"}}", json::jesc(&h.event)))
                    .collect();
                format!(
                    "{{\"name\":\"{}\",\"version\":\"{}\",\"desc\":\"{}\",\"enabled\":{},\"tools\":[{}],\"tasks\":[{}],\"hooks\":[{}]}}",
                    json::jesc(&p.name),
                    json::jesc(&p.version),
                    json::jesc(&p.desc),
                    enabled,
                    tools.join(","),
                    tasks.join(","),
                    hooks.join(",")
                )
            })
            .collect();
        let logsj = logs
            .iter()
            .map(|s| format!("\"{}\"", json::jesc(s)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"ok\":true,\"plugins\":[{}],\"logs\":[{}]}}",
            arr.join(","),
            logsj
        )
    }
}

/// 插件 KV 持久化文件路径。
fn kv_path() -> String {
    let dir = crate::config::Config::panel_dir();
    format!("{}/.vpanel-plugins-kv.json", dir)
}

/// 插件禁用状态持久化文件路径。
fn disabled_path() -> String {
    let dir = crate::config::Config::panel_dir();
    format!("{}/.vpanel-plugins-disabled.json", dir)
}

/// 取加速前缀，保证以 `/` 结尾（与 shop.rs 一致）。
fn accel_of(cfg: &Config) -> String {
    let a = cfg.download.accel.trim().trim_end_matches('/');
    if a.is_empty() {
        crate::shop::DEFAULT_ACCEL.to_string()
    } else {
        format!("{}/", a)
    }
}

/// 插件商店清单远程 URL（仓库 plugins.yml）。
fn store_list_url(cfg: &Config, accel: &str) -> String {
    let repo = cfg.download.store.trim().trim_end_matches('/');
    let (path, branch) = match repo.split_once('@') {
        Some((p, b)) => (p, b),
        None => (repo, "main"),
    };
    format!(
        "{}https://raw.githubusercontent.com/{}/refs/heads/{}/plugins.yml",
        accel, path, branch
    )
}

/// 仓库内任意文件的 raw 下载地址（相对路径）。
fn raw_url(cfg: &Config, file: &str, accel: &str) -> String {
    let repo = cfg.download.store.trim().trim_end_matches('/');
    let (path, branch) = match repo.split_once('@') {
        Some((p, b)) => (p, b),
        None => (repo, "main"),
    };
    format!(
        "{}https://raw.githubusercontent.com/{}/refs/heads/{}/{}",
        accel, path, branch, file
    )
}

/// 内置兜底：商店仓库不可用时为空（在线安装依赖远程仓库）。
fn default_store() -> Vec<StoreItem> {
    Vec::new()
}

/// 极简的本地时间（yyyy-mm-dd hh:mm:ss），避免引入 chrono。
fn chrono_now() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let target = *crate::config::tz();
    let local = s as i64 + target;
    let days = local / 86400;
    let secs = local % 86400;
    // 1970-01-01 起计算年月日（仅粗略，用于日志足够）。
    let (y, m, d) = civil_from_days(days as i64);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y,
        m,
        d,
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dd = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mm = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if mm <= 2 { y + 1 } else { y }), mm, dd)
}