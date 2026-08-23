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
        "tools/list" => tools_list(id_num, state),
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
        format!("\"error\":{{\"code\":-32601,\"message\":\"{}\"}}", payload)
    } else {
        format!("\"result\":{}", payload)
    };
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{},{}}}", id, field)
}

fn init_resp(id: Option<i64>) -> String {
    let result = "{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"vpanel\",\"version\":\"1.3.0\"}}";
    fmt_jsonrpc(
        id,
        result.to_string(),
        false,
    )
}

/// 工具清单。schema 用 r## 保留以便直接拼 JSON。
/// 集成插件工具：每个插件工具暴露为 `p_<plugin>.tool` 形式的 MCP 工具。
fn tools_list(id: Option<i64>, state: &State) -> String {
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
        ("system_info","查看系统信息：OS/内核/CPU型号/内存/磁盘分区/温度","{}"),
        ("list_conns","列出网络连接（TCP），含本地/远端地址与进程","{}"),
        ("kill_conn","结束占用某端口的连接","{\"port\":{\"type\":\"string\"}}"),
        ("list_files","列出目录文件（轻量，name/type/size/human）","{\"path\":{\"type\":\"string\"}}"),
        ("read_file","读取文本文件内容（按需读取）","{\"path\":{\"type\":\"string\"}}"),
        ("delete_path","删除文件或空目录","{\"path\":{\"type\":\"string\"}}"),
        ("write_file","写入文本文件（覆盖）","{\"path\":{\"type\":\"string\"},\"content\":{\"type\":\"string\"}}"),
        ("log_tail","查看文件尾部 n 行日志","{\"file\":{\"type\":\"string\"},\"n\":{\"type\":\"number\"}}"),
        ("disk_top","磁盘占用排行：扫描目录一级子目录占用，参数 path 与 n","{\"path\":{\"type\":\"string\"},\"n\":{\"type\":\"number\"}}"),
        ("list_nginx","列出反向代理/Nginx 站点","{}"),
        ("nginx_add","新增反向代理站点，参数 name/server_name/listen/target","{\"name\":{\"type\":\"string\"},\"server_name\":{\"type\":\"string\"},\"listen\":{\"type\":\"string\"},\"target\":{\"type\":\"string\"}}"),
        ("nginx_toggle","启用/停用反代站点，参数 name 与 enable(布尔)","{\"name\":{\"type\":\"string\"},\"enable\":{\"type\":\"boolean\"}}"),
        ("nginx_delete","删除反代站点，参数 name","{\"name\":{\"type\":\"string\"}}"),
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
    // 追加插件工具（依据工具 params 生成 inputSchema）。
    for p in plugins_snapshot(state) {
        for t in &p.tools {
            let mut s = String::new();
            s.push_str("{\"name\":\"");
            s.push_str(&plugin_tool_name(&p, &t.id));
            s.push_str("\",\"description\":\"[插件 ");
            s.push_str(&p.name);
            s.push_str("] ");
            s.push_str(&json::jesc(&t.desc));
            // 由 params 生成 properties 对象。
            let mut props = Vec::new();
            for pp in &t.params {
                let ty = match pp.r#type.as_str() {
                    "number" => "number",
                    "bool" => "boolean",
                    _ => "string",
                };
                props.push(format!(
                    "\"{}\":{{\"type\":\"{}\",\"description\":\"{}\"}}",
                    json::jesc(&pp.id),
                    ty,
                    json::jesc(if pp.desc.is_empty() { &pp.name } else { &pp.desc })
                ));
            }
            s.push_str(&format!(
                "\",\"inputSchema\":{{\"type\":\"object\",\"properties\":{{{}}}}}",
                props.join(",")
            ));
            s.push('}');
            item.push(s);
        }
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
    let path = arg_str(&args, "path");
    let content = arg_str(&args, "content");
    let file = arg_str(&args, "file");
    let n: usize = arg_str(&args, "n").parse().unwrap_or(20);
    let server_name = arg_str(&args, "server_name");
    let listen = arg_str(&args, "listen");
    let target = arg_str(&args, "target");
    let fname = arg_str(&args, "name");
    let enable = arg_str(&args, "enable") == "true" || arg_str(&args, "enable") == "1";

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
        "system_info" => (true, crate::extra::sysinfo_json()),
        "list_conns" => (true, crate::extra::conns_json()),
        "kill_conn" => {
            let (o, m) = crate::extra::conn_kill(&port);
            (o, m)
        }
        "list_files" => {
            let p = if path.is_empty() { "/".to_string() } else { path.clone() };
            (true, crate::extra::ls_json(&p))
        }
        "read_file" => (true, crate::extra::read_file_json(&path)),
        "delete_path" => {
            let (o, m) = crate::extra::del_path(&path);
            (o, m)
        }
        "write_file" => {
            let (o, m) = crate::extra::write_file(&path, content.as_bytes());
            (o, m)
        }
        "log_tail" => {
            let p = if file.is_empty() { "/var/log/syslog".to_string() } else { file.clone() };
            (true, crate::extra::log_tail_json(&p, n))
        }
        "disk_top" => {
            let p = if path.is_empty() { "/".to_string() } else { path.clone() };
            (true, crate::extra::disk_top_json(&p, n))
        }
        "list_nginx" => (true, crate::nginx::nginx_list_json()),
        "nginx_add" => {
            let (o, m) = crate::nginx::nginx_add(&fname, &server_name, &listen, &target);
            (o, m)
        }
        "nginx_toggle" => {
            let (o, m) = crate::nginx::nginx_toggle(&fname, enable);
            (o, m)
        }
        "nginx_delete" => {
            let (o, m) = crate::nginx::nginx_delete(&fname);
            (o, m)
        }
        _ => plugin_or_unknown(state, name.as_str(), &args),
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

/// 插件工具对应的 MCP 工具名：`p_<插件名>_<工具id>`（不做特殊字符）。
fn plugin_tool_name(p: &crate::plugins::Plugin, tool: &str) -> String {
    format!("p_{}_{}", skill(&p.name), skill(tool))
}

/// 只保留 [A-Za-z0-9_]，其余转成 `_`。
fn skill(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

/// 插件快照（避免持有锁过久）。
fn plugins_snapshot(state: &State) -> Vec<crate::plugins::Plugin> {
    state.plugins.snapshot()
}

/// 若非内置工具，尝试按插件工具派发；找不到则返回“未知工具”。
fn plugin_or_unknown(state: &State, name: &str, args: &str) -> (bool, String) {
    let amap = parse_json_obj(args);
    for p in plugins_snapshot(state) {
        for t in &p.tools {
            if plugin_tool_name(&p, &t.id) == name {
                return state.plugins.call_tool(&p.name, &t.id, amap);
            }
        }
    }
    (false, format!("未知工具: {}", name))
}

/// 宽松解析 `{"k1":"v1","k2":123,...}` 顶层对象，返回键值对（非字符串值转文本）。
fn parse_json_obj(s: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let body = s.trim();
    let body = body.strip_prefix('{').unwrap_or(body);
    let body = body.strip_suffix('}').unwrap_or(body);
    let mut i = 0;
    let b = body.as_bytes();
    while i < b.len() {
        // 找 key 引号
        while i < b.len() && b[i] != b'"' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        i += 1;
        let ks = i;
        while i < b.len() && b[i] != b'"' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let key = body[ks..i].to_string();
        i += 1;
        // 跳过冒号与空白
        while i < b.len() && (b[i] == b':' || b[i].is_ascii_whitespace()) {
            i += 1;
        }
        // 值：字符串或裸值
        if i < b.len() && b[i] == b'"' {
            i += 1;
            let vs = i;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            out.insert(key, unescape_str(&body[vs..i.min(body.len())]));
            i += 1;
        } else {
            let vs = i;
            while i < b.len() && b[i] != b',' && b[i] != b'}' {
                i += 1;
            }
            out.insert(key, body[vs..i.min(body.len())].trim().to_string());
        }
        // 跳过逗号
        while i < b.len() && b[i] != b'"' && b[i] != b'{' {
            i += 1;
        }
    }
    out
}

fn unescape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}