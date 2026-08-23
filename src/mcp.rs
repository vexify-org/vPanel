//! 内置 MCP（Model Context Protocol）端点，供 AI 客户端直接调用面板能力。
//!
//! 采用 JSON-RPC 2.0 over HTTP，POST `/mcp` 提交请求、返回 JSON 结果。
//! 当前实现了 MCP 的核心方法：
//!   - `initialize`          协商协议版本
//!   - `tools/list`          列出可调用工具
//!   - `tools/call`          执行工具并返回运行结果
//!   - `notifications/initialized`  空确认
//!
//! 内存策略：按请求一次性处理，少量临时 String，随响应结束即释放。

use crate::http::State;
use crate::json;

/// 处理一次 MCP 请求。`body` 是原始请求体，返回 JSON 响应字节。
pub fn handle(body: &[u8], state: &State) -> Vec<u8> {
    let text = String::from_utf8_lossy(body);
    let req = json::parse_json(&text);

    // 提取 id（可为 number 或 string）。
    let id = req.get("id").cloned().unwrap_or_default();
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
    let params = req.get("params").cloned().unwrap_or_default();

    let resp = match method.as_str() {
        "initialize" => {
            let mcp_server = params.get("clientInfo").and_then(|c| c.get("name")).and_then(|c| c.as_str()).unwrap_or("UNKNOWN");
            let _ = mcp_server;
            json_obj! {
                "jsonrpc" => "2.0",
                "id" => id,
                "result" => {
                    "protocolVersion" => "2025-06-18",
                    "capabilities" => {
                        "tools" => {}
                    },
                    "serverInfo" => { "name" => "vpanel-mcp", "version" => "1.0.0" }
                }
            }
        }
        "notifications/initialized" => {
            json_obj! { "jsonrpc" => "2.0", "id" => id }
        }
        "tools/list" => {
            json_obj! {
                "jsonrpc" => "2.0",
                "id" => id,
                "result" => { "tools" => tool_list() }
            }
        }
        "tools/call" => {
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            let arguments = params.get("arguments").cloned().unwrap_or_default();
            let (result, is_err) = call_tool(&name, &arguments, state);
            if is_err {
                json_obj! {
                    "jsonrpc" => "2.0",
                    "id" => id,
                    "result" => {
                        "content" => [ { "type" => "text", "text" => result } ],
                        "isError" => true
                    }
                }
            } else {
                json_obj! {
                    "jsonrpc" => "2.0",
                    "id" => id,
                    "result" => {
                        "content" => [ { "type" => "text", "text" => result } ]
                    }
                }
            }
        }
        _ => {
            json_obj! {
                "jsonrpc" => "2.0",
                "id" => id,
                "error" => { "code" => -32601, "message" => format!("method not found: {}", method) }
            }
        }
    };
    resp.into_bytes()
}

/// 定义暴露给 AI 的工具清单。
fn tool_list() -> Vec<crate::json::JVal> {
    let t = |name: &str, desc: &str, schema: crate::json::JVal| {
        crate::json::JVal::Obj(json_map! {
            "name" => JStr(name),
            "description" => JStr(desc),
            "inputSchema" => schema,
        })
    };
    let obj = |props: Vec<(&str, &str, bool)>| {
        let mut pmap = crate::json::JsonMap::new();
        for (k, v, _) in props.iter() {
            pmap.insert(k.to_string(), crate::json::JVal::from(v.to_string()));
        }
        crate::json::JVal::obj(&[
            ("type", "object"),
            ("properties", ""),
        ])
    };
    let _ = obj;
    let string_schema = |desc: &str| crate::json::JVal::obj(&[("type", "string"), ("description", desc)]);
    let empty = crate::json::JVal::obj(&[("type", "object"), ("properties", "")]);

    let props_t = |fields: &[(&str, &str)]| {
        let mut pmap = crate::json::JsonMap::new();
        for (k, d) in fields {
            pmap.insert((*k).to_string(), crate::json::JVal::obj(&[("type", "string"), ("description", *d)]));
        }
        crate::json::JVal::obj(&[
            ("type", "object"),
            ("properties", crate::json::JVal::from_json_map(pmap)),
        ])
    };

    vec![
        t("system_status", "读取系统实时状态：CPU/内存/磁盘/网络/负载", empty.clone()),
        t("process_list", "获取进程列表（按内存排序，前 80）", empty.clone()),
        t("process_kill", "结束一个进程，参数 pid", props_t(&[("pid", "进程 PID")])),
        t("service_list", "获取系统服务列表", empty.clone()),
        t("service_action", "启动/停止/重启服务，参数 name, action", props_t(&[("name", "服务名"), ("action", "start|stop|restart")])),
        t("firewall_list", "获取防火墙放行规则", empty.clone()),
        t("firewall_add", "放行端口，参数 port", props_t(&[("port", "端口或 端口/协议")])),
        t("firewall_del", "删除放行端口，参数 port", props_t(&[("port", "端口或 端口/协议")])),
        t("task_list", "获取定时任务列表", empty.clone()),
        t("task_add", "新增定时任务，参数 schedule, command", props_t(&[("schedule", "5 段 cron"), ("command", "命令")])),
        t("shop_list", "获取软件商店可安装软件列表", empty.clone()),
        t("shop_install", "一键安装软件，参数 app_id", props_t(&[("app_id", "软件 id，来自 shop_list")])),
        t("shop_accel_check", "检测下载加速源是否可达", empty.clone()),
    ]
}

/// 执行一个工具，返回 (文本结果, 是否错误)。
fn call_tool(name: &str, args: &crate::json::JVal, state: &State) -> (String, bool) {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    #[allow(unused_assignments)]
    let mut out: String;
    match name {
        "system_status" => (crate::system::system_json(&state.monitor), false),
        "process_list" => (crate::system::processes_json(), false),
        "process_kill" => {
            let pid: u32 = s("pid").trim().parse().unwrap_or(0);
            if pid == 0 {
                ("缺少有效 pid".into(), true)
            } else {
                let ok = crate::system::kill_pid(pid);
                (format!("kill 结果: ok={}", ok), !ok)
            }
        }
        "service_list" => (crate::ctl::services_json(), false),
        "service_action" => {
            let name = s("name");
            let action = s("action");
            if name.is_empty() || !matches!(action.as_str(), "start" | "stop" | "restart") {
                ("需要 name 与 action(start/stop/restart)".into(), true)
            } else {
                let (ok, msg) = crate::ctl::service_action(&name, &action);
                let txt = if msg.is_empty() { format!("service {} {}", action, name) } else { format!("service {} {}: {}", action, name, msg) };
                (txt, !ok)
            }
        }
        "firewall_list" => (crate::ctl::firewall_json(), false),
        "firewall_add" => {
            let port = s("port");
            if port.is_empty() {
                ("缺少 port".into(), true)
            } else {
                let (ok, msg) = crate::ctl::fw_allow(&port);
                (if msg.is_empty() { format!("已放行 {}", port) } else { msg }, !ok)
            }
        }
        "firewall_del" => {
            let port = s("port");
            if port.is_empty() {
                ("缺少 port".into(), true)
            } else {
                let (ok, msg) = crate::ctl::fw_delete(&port);
                (if msg.is_empty() { format!("已删除 {}", port) } else { msg }, !ok)
            }
        }
        "task_list" => (crate::ctl::tasks_json(), false),
        "task_add" => {
            let sch = s("schedule");
            let cmd = s("command");
            let (ok, msg) = crate::ctl::task_add(&sch, &cmd);
            (if ok { format!("已添加任务: {} {}", sch, cmd) } else { msg }, !ok)
        }
        "shop_list" => (crate::shop::shop_json(&state.cfg), false),
        "shop_install" => {
            let app = s("app_id");
            if app.is_empty() {
                ("缺少 app_id（参见 shop_list）".into(), true)
            } else {
                let (ok, msg) = crate::shop::install(&app, &state.cfg);
                (if ok { format!("安装成功率启动，输出: {}", msg) } else { format!("安装失败: {}", msg) }, !ok)
            }
        }
        "shop_accel_check" => (crate::shop::accel_check(&state.cfg), false),
        _ => (format!("未知工具: {}", name), true),
    }
}

#[allow(unused_variables)]
fn _unused(_: &crate::json::JVal, x: &crate::json::JVal) {}

macro_rules! json_obj {
    ( $( $k:expr => $v:expr ),+ $(,)? ) => {{
        let mut m = crate::json::JsonMap::new();
        $( m.insert($k.to_string(), crate::json::JVal::from($v)); )+
        crate::json::JVal::from_json_map(m)
    }};
}
use json_obj;

macro_rules! json_map {
    ( $( $k:expr => $v:expr ),+ $(,)? ) => {{
        use crate::json::JVal;
        let mut m = crate::json::JsonMap::new();
        $( m.insert($k.to_string(), $v); )+
        m
    }};
}
use json_map;

type JStr = crate::json::JVal;