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

/// 一个工具：暴露成面板 API 的一键能力。
#[derive(Debug, Clone, Deserialize)]
pub struct Tool {
    pub id: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub script: String,
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

/// 共享的插件注册表 + 最近日志（有界）。
pub struct Plugins {
    list: Mutex<Vec<Plugin>>,
    logs: Mutex<VecDeque<String>>,
    task_fire: Mutex<std::collections::HashMap<String, std::time::Instant>>,
}

/// 命令 / fetch 的系统执行能力的实现。
struct CmdBuiltin;

impl Builtin for CmdBuiltin {
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
}

impl Plugins {
    pub fn new() -> Arc<Plugins> {
        Arc::new(Plugins {
            list: Mutex::new(Vec::new()),
            logs: Mutex::new(VecDeque::new()),
            task_fire: Mutex::new(std::collections::HashMap::new()),
        })
    }

    /// 插件快照（clone，避免调用方持有锁）。
    pub fn snapshot(&self) -> Vec<Plugin> {
        self.list.lock().unwrap().clone()
    }

    /// 从配置指定的目录加载全部 `*.yml` 插件。
    pub fn load(self: &Arc<Self>, cfg: &Config) {
        let dir = cfg.plugins.dir.clone();
        let read = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => {
                eprintln!("panel: 插件目录 {} 不存在，已忽略", dir);
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
        self.run_hooks("on_init");
        self.spawn_scheduler();
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
                for t in &p.tasks {
                    let key = format!("{}/{}", p.name, t.id);
                    if this.due(&key, t.every.max(1), now) {
                        this.exec_script(&p.name, &t.script, &format!("[task {}]", t.id));
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
    pub fn exec_script(&self, tag: &str, script: &str, label: &str) -> (bool, String) {
        let mut it = Interp::new(&CmdBuiltin);
        let out = match it.run(script) {
            Ok(o) => o,
            Err(e) => {
                self.push_log(tag, format!("脚本错误: {}", e));
                return (false, e);
            }
        };
        for l in it.logs() {
            self.push_log(tag, l.clone());
        }
        let msg = if out.is_empty() {
            format!("{} 已执行", label)
        } else {
            match label.is_empty() {
                true => out,
                false => format!("{}: {}", label, out),
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
            for h in &p.hooks {
                if h.event == event {
                    let _ = self.exec_script(&p.name, &h.script, &format!("[hook {}]", event));
                }
            }
        }
    }

    /// 调用某个插件的某个工具。
    pub fn call_tool(&self, plugin: &str, tool: &str) -> (bool, String) {
        let script = {
            let list = self.list.lock().unwrap();
            list.iter()
                .filter(|p| p.name == plugin)
                .flat_map(|p| p.tools.iter())
                .find(|t| t.id == tool)
                .map(|t| t.script.clone())
        };
        match script {
            Some(s) => self.exec_script(plugin, &s, &format!("[tool {}]", tool)),
            None => (false, format!("未找到插件 {}/{}", plugin, tool)),
        }
    }

    /// 插件列表 JSON（供前端 /api/plugins）。
    pub fn list_json(&self) -> String {
        let list = self.list.lock().unwrap();
        let logs: Vec<String> = self.logs.lock().unwrap().iter().rev().take(30).cloned().collect();
        let arr: Vec<String> = list
            .iter()
            .map(|p| {
                let tools: Vec<String> = p
                    .tools
                    .iter()
                    .map(|t| {
                        format!(
                            "{{\"id\":\"{}\",\"desc\":\"{}\"}}",
                            json::jesc(&t.id),
                            json::jesc(&t.desc)
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
                    "{{\"name\":\"{}\",\"version\":\"{}\",\"desc\":\"{}\",\"tools\":[{}],\"tasks\":[{}],\"hooks\":[{}]}}",
                    json::jesc(&p.name),
                    json::jesc(&p.version),
                    json::jesc(&p.desc),
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