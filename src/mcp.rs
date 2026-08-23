//! 内置 MCP（Model Context Protocol）端点，供 AI 客户端调用面板能力。
//!
//! 采用 MCP Streamable HTTP 传输中最常用的一小部分：POST /mcp 上的 JSON-RPC
//! 方法 `initialize`、`tools/list`、`tools/call`。请求/响应均为 JSON-RPC 2.0，
//! 便于 Claude / Cursor 等客户端接入。
//!
//! tools/call 的参数从请求里的 `arguments:{...}` 对象中读取。

use crate::http::State;
use crate::json;

pub fn handle(body: &[u8], state: &State) -> Vec<u8> {
    let text = String::from_utf8_lossy(body);
    // id 可能同时作为 "id" 字段，统一定位。
    let method = str_field(&text, "method").unwrap_or("").to_string();
    let id_num = num_field(&text, "id");

    let resp = match method.as_str() {
        "initialize" => init_resp(id_num),
        "tools/list" => tools_list(id_num),
        "tools/call" => tools_call(&text, state, id_num),
        "ping" => fmt_jsonrpc(id_num, "{\"result\":{}}".into(), false),
        _ => fmt_jsonrpc(
            id_num,
            json::jesc("method not found").into(),
            true,
        ),
    };
    if resp.is_empty() {
        return Vec::new();
    }
    resp.into_bytes()
}

/// 拼装一个 JSON-RPC 响应。error=true 时返回 error 对象，否则 result。
fn fmt_jsonrpc(id: Option<i64>, payload: String, is_error: bool) -> String {
    let id = match id {
        Some(n) => n.to_string(),
        None => "null".to_string(),
    };
    let field = if is_error {
        format!("error:{{\"code\":-32601,\"message\":\"{}\"}}", payload)
    } else {
        format!("result:{}", payload)
    };
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{},{}}}", id, field)
}

fn init_resp(id: Option<i64>) -> String {
    let result = "{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"vpanel\",\"version\":\"1.0.0\"}}";
    fmt_jsonrpc(
        id,
        result.to_string(),
        false,
    )
}

/// 工具清单。schema 用 r## 保留以便直接拼 JSON。
fn tools_list(id: Option<i64>) -> String {
    let tools = [
        ("system_overview","查看系统状态：CPU、内存、磁盘、网络、负载","{}"),
        ("list_processes","列出进程（按内存前 80）","{}"),
        ("kill_process","结束进程","{\"pid\":{\"type\":\"number\"}}"),
        ("list_services","列出系统服务","{}"),
        ("service_action","启动/停止/重启服务，参数 name 与 action=start|stop|restart","{\"name\":{\"type\":\"string\"},\"action\":{\"type\":\"string\"}}"),
        ("list_firewall","列出防火墙放行规则","{}"),
        ("firewall_add","放行端口","{\"port\":{\"type\":\"string\"}}"),
        ("firewall_del","删除端口放行","{\"port\":{\"type\":\"string\"}}"),
        ("list_tasks","列出定时任务","{}"),
        ("task_add","添加定时任务，参数 schedule(5段cron) 与 command","{\"schedule\":{\"type\":\"string\"},\"command\":{\"type\":\"string\"}}"),
    ];
    let mut item = Vec::new();
    for (name, desc, schema) in tools {
        item.push(format!(
            "{{\"name\":\"{}\",\"description\":\"{}\",\"inputSchema\":{{\"type\":\"object\",\"properties\":{}}}}}",
            name,
            json::jesc(desc),
            schema
        ));
    }
    let result = format!(
        "{{\"tools\":[{}]}}",
        item.join(",")
    );
    fmt_jsonrpc(id, result, false)
}

/// tools/call：执行工具，参数从 arguments 对象读取。
fn tools_call(body: &str, state: &State, id: Option<i64>) -> String {
    // MCP tools/call: params.name 是工具名；params.arguments 是参数。
    // 由于 params 出现在 arguments 之前，首个 "name" 即工具名。
    let name = str_field(body, "name").unwrap_or("").to_string();
    // 解析 arguments 对象里的各字段。
    let args = extract_arg_json(body);
    let pid = arg_str(&args, "pid");
    let svc_name = arg_str(&args, "name");
    let action = arg_str(&args, "action");
    let port = arg_str(&args, "port");
    let schedule = arg_str(&args, "schedule");
    let command = arg_str(&args, "command");

    let (ok, text) = match name.as_str() {
        "system_overview" => (true, crate::system::system_json(&state.monitor)),
        "list_processes" => (true, crate::system::processes_json()),
        "list_services" => (true, crate::ctl::services_json()),
        "list_firewall" => (true, crate::ctl::firewall_json()),
        "list_tasks" => (true, crate::ctl::tasks_json()),
        "service_action" => crate::ctl::service_action(&svc_name, &action),
        "firewall_add" => {
            let (o, m) = crate::ctl::fw_allow(&port);
            (o, m)
        }
        "firewall_del" => {
            let (o, m) = crate::ctl::fw_delete(&port);
            (o, m)
        }
        "task_add" => {
            let (o, m) = crate::ctl::task_add(&schedule, &command);
            (o, m)
        }
        "kill_process" => match pid.trim().parse::<u32>() {
            Ok(p) => (crate::system::kill_pid(p), format!("请求结束进程 {}", p)),
            Err(_) => (false, "pid 需为数字".into()),
        },
        _ => (false, format!("未知工具: {}", name)),
    };
    let result = format!(
        "{{\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}],\"isError\":{}}}",
        json::jesc(&text),
        if ok { "false" } else { "true" }
    );
    fmt_jsonrpc(id, result, false)
}

/// 从 JSON 里取字符串字段（首个出现处）。
fn str_field<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\":", key);
    let idx = body.find(&needle)?;
    let rest = body[idx + needle.len()..].trim_start();
    if rest.starts_with('"') {
        let i = rest[1..].find('"')?;
        Some(&rest[1..1 + i])
    } else {
        None
    }
}

/// 从 JSON 里取数字字段（首个出现处）。
fn num_field(body: &str, key: &str) -> Option<i64> {
    let needle = format!("\"{}\":", key);
    let idx = body.find(&needle)?;
    let rest = body[idx + needle.len()..].trim_start();
    let e = rest
        .find(|c: char| c == ',' || c == '}' || c == ' ' || c == '\n')
        .unwrap_or(rest.len());
    rest[..e].trim().parse().ok()
}

/// 抽取 `arguments:{...}` 子串（简单括号匹配）。
fn extract_arg_json(body: &str) -> String {
    let key = "\"arguments\":";
    let start = match body.find(key) {
        Some(i) => i + key.len(),
        None => return String::new(),
    };
    let rest = body[start..].trim_start();
    if !rest.starts_with('{') {
        return String::new();
    }
    let mut depth = 0i32;
    let mut in_str = false;
    for (off, b) in rest.bytes().enumerate() {
        match b {
            b'"' if !in_str => in_str = true,
            b'"' if in_str => in_str = false,
            b'{' if !in_str => depth += 1,
            b'}' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    return rest[..=off].to_string();
                }
            }
            _ => {}
        }
    }
    String::new()
}

/// 在 arguments JSON 里取字符串键值。
fn arg_str(args: &str, key: &str) -> String {
    let needle = format!("\"{}\":", key);
    let idx = match args.find(&needle) {
        Some(i) => i + needle.len(),
        None => return String::new(),
    };
    let rest = args[idx..].trim_start();
    if rest.starts_with('"') {
        let i = match rest[1..].find('"') {
            Some(j) => j,
            None => return String::new(),
        };
        rest[1..1 + i].to_string()
    } else {
        // 数字/布尔
        let e = rest
            .find(|c: char| c == ',' || c == '}')
            .unwrap_or(rest.len());
        rest[..e].trim().to_string()
    }
}