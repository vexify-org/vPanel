//! /api/* 路由：数据查询（GET）与系统操作（POST）。
//! 统一返回 JSON。操作成功返回 {"ok":true}，失败返回 {"ok":false,"msg":...}。

use crate::http::State;
use crate::json;

/// 派发 API 请求。
pub fn route(method: &str, target: &str, body: &[u8], state: &State) -> Vec<u8> {
    // 拆分 路径?查询串 与 裸查询参数。
    let trg = target.split('?').next().unwrap_or(target);
    let qs = target.splitn(2, '?').nth(1).unwrap_or("");
    let resp = match trg {
        "/api/system" => crate::system::system_json(&state.monitor),
        "/api/processes" => crate::system::processes_json(),
        "/api/services" => crate::ctl::services_json(),
        "/api/firewall" => crate::ctl::firewall_json(),
        "/api/tasks" => crate::ctl::tasks_json(),
        "/api/plugins" => state.plugins.list_json(),
        "/api/plugin/kv" => state.plugins.kv_list_json(),
        "/api/info" => crate::extra::sysinfo_json(),
        "/api/conns" => crate::extra::conns_json(),
        "/api/files" => {
            let q = question_query(qs);
            let path = q_get(&q, "path").map(str::to_string).unwrap_or_else(|| "/".to_string());
            crate::extra::ls_json(&path)
        }
        "/api/file/read" => {
            let q = question_query(qs);
            match q_get(&q, "path") {
                Some(p) => crate::extra::read_file_json(p),
                None => err("缺少 path"),
            }
        }
        "/api/log/tail" => {
            let q = question_query(qs);
            let file = q_get(&q, "file").unwrap_or("").to_string();
            let n: usize = q_get(&q, "n").and_then(|x| x.parse().ok()).unwrap_or(100);
            if file.is_empty() {
                err("缺少 file")
            } else {
                crate::extra::log_tail_json(&file, n)
            }
        }
        "/api/log/follow" => {
            let q = question_query(qs);
            let file = q_get(&q, "file").unwrap_or("").to_string();
            let pos: u64 = q_get(&q, "pos").and_then(|x| x.parse().ok()).unwrap_or(0);
            if file.is_empty() {
                err("缺少 file")
            } else {
                crate::extra::log_follow_json(&file, pos)
            }
        }
        "/api/plugin/store" => state.plugins.store_list_json(&state.cfg),
        "/api/plugin/store/install" => {
            if method != "POST" {
                err("需要 POST")
            } else {
                let fields = json::parse_form(body);
                let id = json::form_get(&fields, "id").unwrap_or("").trim().to_string();
                if id.is_empty() {
                    err("缺少 id")
                } else {
                    let (ok, msg) = state.plugins.store_install(&id, &state.cfg);
                    ok_bool(ok, msg)
                }
            }
        }
        target if target.starts_with("/api/plugin/") => plugin_call(target, body, state),
        "/api/shop" => state.shop.list_json(&state.cfg),
        "/api/shop/install" => {
            if method != "POST" {
                err("需要 POST")
            } else {
                let fields = json::parse_form(body);
                let id = json::form_get(&fields, "id").unwrap_or("").trim().to_string();
                if id.is_empty() {
                    err("缺少 id")
                } else {
                    let (ok, msg, _exists) = state.shop.install(&id, &state.cfg);
                    ok_bool(ok, msg)
                }
            }
        }
        target if target.starts_with("/api/plugin/") => plugin_call(target, body, state),
        _ => {
            if method == "POST" {
                action_route(trg, body, qs)
            } else {
                err("未知接口")
            }
        }
    };
    resp.into_bytes()
}

/// 解析 URL 查询串（'&' 分隔、'=' 赋值）为键值映射。
fn question_query(qs: &str) -> Vec<(String, String)> {
    qs.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (pct_dec(k), pct_dec(v)),
            None => (pct_dec(p), String::new()),
        })
        .collect()
}

/// 查询串字段取值。
fn q_get<'a>(q: &'a [(String, String)], key: &str) -> Option<&'a str> {
    q.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// 极简 percent 解码（+ 视为空格）。
fn pct_dec(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let h = (b[i + 1] as char).to_digit(16);
                let l = (b[i + 2] as char).to_digit(16);
                if let (Some(h), Some(l)) = (h, l) {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                    continue;
                }
                out.push(b[i]);
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `/api/plugin/<plugin>/<tool>`：调用插件工具，可选入参（body 为表单）。
/// 特殊尾部动作：`enable` / `disable` / `uninstall`。
fn plugin_call(target: &str, body: &[u8], state: &State) -> String {
    let rest = &target["/api/plugin/".len()..];
    let mut it = rest.splitn(2, '/');
    let first = it.next().unwrap_or("").trim().to_string();
    let second = it.next().unwrap_or("").trim().to_string();
    if first.is_empty() {
        return err("用法: /api/plugin/<插件名>/<工具id>");
    }
    // 启用 / 禁用 / 卸载
    match second.as_str() {
        "enable" => {
            let (ok, msg) = state.plugins.set_enabled(&first, true);
            return ok_bool(ok, msg);
        }
        "disable" => {
            let (ok, msg) = state.plugins.set_enabled(&first, false);
            return ok_bool(ok, msg);
        }
        "uninstall" => {
            let (ok, msg) = state.plugins.store_uninstall(&first, &state.cfg);
            return ok_bool(ok, msg);
        }
        _ => {}
    }
    let plugin = first;
    let tool = second;
    if tool.is_empty() {
        return err("用法: /api/plugin/<插件名>/<工具id>");
    }
    let fields = json::parse_form(body);
    let mut args = std::collections::HashMap::new();
    for (k, v) in fields {
        if k != "id" {
            args.insert(k, v);
        }
    }
    let (ok, msg) = state.plugins.call_tool(&plugin, &tool, args);
    ok_bool(ok, msg)
}

/// 处理各类操作类 POST。
fn action_route(target: &str, body: &[u8], _qs: &str) -> String {
    let fields = json::parse_form(body);
    match target {
        "/api/conn/kill" => {
            let port = json::form_get(&fields, "port").unwrap_or("").trim().to_string();
            if port.is_empty() {
                return err("缺少 port");
            }
            let (ok, msg) = crate::extra::conn_kill(&port);
            ok_bool(ok, msg)
        }
        "/api/file/delete" => {
            let path = json::form_get(&fields, "path").unwrap_or("").trim().to_string();
            if path.is_empty() {
                return err("缺少 path");
            }
            let (ok, msg) = crate::extra::del_path(&path);
            ok_bool(ok, msg)
        }
        "/api/file/save" => {
            // 保存文本：path 走表单，data 为文本内容。
            let path = json::form_get(&fields, "path").unwrap_or("").trim().to_string();
            let data = json::form_get(&fields, "data").unwrap_or("").to_string();
            if path.is_empty() {
                return err("缺少 path");
            }
            let (ok, msg) = crate::extra::write_file(&path, data.as_bytes());
            ok_bool(ok, msg)
        }
        "/api/file/upload" => {
            // 上传二进制：path/target 由查询串指定，body 为文件原始字节。
            let path = q_get(&question_query(_qs), "path")
                .map(str::to_string)
                .unwrap_or_default();
            if path.is_empty() {
                return err("缺少 path（通过 ?path= 指定目标文件）");
            }
            if body.len() > 8 * 1024 * 1024 {
                return err("文件过大（上限 8MB）");
            }
            let (ok, msg) = crate::extra::write_file(&path, body);
            ok_bool(ok, msg)
        }
        "/api/process/kill" => {
            let pid: u32 = json::form_get(&fields, "pid")
                .and_then(|p| p.trim().parse().ok())
                .unwrap_or(0);
            if pid == 0 {
                return err("缺少有效 pid");
            }
            ok_bool(crate::system::kill_pid(pid), format!("已请求结束进程 {}", pid))
        }
        "/api/service/action" => {
            let name = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let action = json::form_get(&fields, "action").unwrap_or("").trim().to_string();
            if name.is_empty() || !matches!(action.as_str(), "start" | "stop" | "restart") {
                return err("缺少 name 或 action(应为 start/stop/restart)");
            }
            let (ok, msg) = crate::ctl::service_action(&name, &action);
            ok_bool(ok, msg)
        }
        "/api/firewall/add" => {
            let port = json::form_get(&fields, "port").unwrap_or("").trim().to_string();
            if port.is_empty() {
                return err("缺少 port");
            }
            let (ok, msg) = crate::ctl::fw_allow(&port);
            ok_bool(ok, msg)
        }
        "/api/firewall/del" => {
            let port = json::form_get(&fields, "port").unwrap_or("").trim().to_string();
            if port.is_empty() {
                return err("缺少 port");
            }
            let (ok, msg) = crate::ctl::fw_delete(&port);
            ok_bool(ok, msg)
        }
        "/api/tasks/add" => {
            let schedule = json::form_get(&fields, "schedule").unwrap_or("").trim().to_string();
            let command = json::form_get(&fields, "command").unwrap_or("").trim().to_string();
            let (ok, msg) = crate::ctl::task_add(&schedule, &command);
            ok_bool(ok, msg)
        }
        _ => err("未知操作"),
    }
}

fn ok_bool(ok: bool, msg: String) -> String {
    if ok {
        format!("{{\"ok\":true,\"msg\":\"{}\"}}", json::jesc(&msg))
    } else {
        format!("{{\"ok\":false,\"msg\":\"{}\"}}", json::jesc(&msg))
    }
}

fn err(msg: &str) -> String {
    format!("{{\"ok\":false,\"msg\":\"{}\"}}", json::jesc(msg))
}