//! /api/* 路由：数据查询（GET）与系统操作（POST）。
//! 统一返回 JSON。操作成功返回 {"ok":true}，失败返回 {"ok":false,"msg":...}。

use crate::http::State;
use crate::json;

/// 派发 API 请求。
pub fn route(method: &str, target: &str, body: &[u8], state: &State) -> Vec<u8> {
    let resp = match target {
        "/api/system" => crate::system::system_json(&state.monitor),
        "/api/processes" => crate::system::processes_json(),
        "/api/services" => crate::ctl::services_json(),
        "/api/firewall" => crate::ctl::firewall_json(),
        "/api/tasks" => crate::ctl::tasks_json(),
        "/api/plugins" => state.plugins.list_json(),
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
        target if target.starts_with("/api/plugin/") => plugin_call(target, state),
        _ => {
            if method == "POST" {
                action_route(target, body)
            } else {
                err("未知接口")
            }
        }
    };
    resp.into_bytes()
}

/// `/api/plugin/<plugin>/<tool>`：调用插件工具。
fn plugin_call(target: &str, state: &State) -> String {
    let rest = &target["/api/plugin/".len()..];
    let mut it = rest.splitn(2, '/');
    let plugin = it.next().unwrap_or("").trim().to_string();
    let tool = it.next().unwrap_or("").trim().to_string();
    if plugin.is_empty() || tool.is_empty() {
        return err("用法: /api/plugin/<插件名>/<工具id>");
    }
    let (ok, msg) = state.plugins.call_tool(&plugin, &tool);
    ok_bool(ok, msg)
}

/// 处理各类操作类 POST。
fn action_route(target: &str, body: &[u8]) -> String {
    let fields = json::parse_form(body);
    match target {
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