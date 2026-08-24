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
        "/api/autostart" => crate::nginx::autostart_json(),
        "/api/nginx" => crate::nginx::nginx_list_json(),
        "/api/website" => crate::website::website_list_json(),
        "/api/disk/top" => {
            let q = question_query(qs);
            let path = q_get(&q, "path").unwrap_or("/").to_string();
            let n: usize = q_get(&q, "n").and_then(|x| x.parse().ok()).unwrap_or(20);
            crate::extra::disk_top_json(&path, n)
        }
        "/api/top" => {
            let q = question_query(qs);
            let n: usize = q_get(&q, "n").and_then(|x| x.parse().ok()).unwrap_or(20);
            crate::extra::resources_top_json(n)
        }
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
        "/api/db/status" => db_status_json(&state.cfg),
        "/api/db/databases" => dbj2(crate::db::databases(&state.cfg.database)),
        "/api/db/users" => dbj2(crate::db::users(&state.cfg.database)),
        "/api/db/backups" => db_backups_json(&state.cfg),
        "/api/ssl" => crate::ssl::list_json(&state.cfg.certs),
        "/api/env" => crate::env::status_json(),
        "/api/backup" => crate::backup::list_json(&state.cfg),
        "/api/security/bans" => crate::security::bans_json(),
        "/api/security/hardening" => crate::security::hardening_status(),
        "/api/security/waf" => crate::extra::waf_status_json(&state.cfg),
        "/api/monitor" => {
            let q = question_query(qs);
            let n: usize = q_get(&q, "n").and_then(|x| x.parse().ok()).unwrap_or(120);
            crate::monitor::monitor_json(n)
        }
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
        "/api/iota" => state.iota.list_json(),
        "/api/iota/status" => {
            let q = question_query(qs);
            let name = q_get(&q, "name").unwrap_or("").trim().to_string();
            if name.is_empty() {
                err("缺少 name")
            } else {
                state.iota.status_json(&name)
            }
        }
        "/api/iota/log" => {
            let q = question_query(qs);
            let name = q_get(&q, "name").unwrap_or("").trim().to_string();
            let n: usize = q_get(&q, "n").and_then(|x| x.parse().ok()).unwrap_or(40);
            if name.is_empty() {
                err("缺少 name")
            } else {
                state.iota.log_tail_json(&name, n)
            }
        }
        "/api/iota/start" => iota_name_op(state, body, |m, n| m
            .start(n)
            .map(|(_, p)| (true, format!("插件 {} 已启动（端口 {p}）", n)))
            .unwrap_or_else(|e| (false, e))),
        "/api/iota/stop" => iota_name_op(state, body, |m, n| m.stop(n)),
        "/api/iota/restart" => iota_name_op(state, body, |m, n| m.restart(n)),
        "/api/iota/keepalive" => iota_keepalive_op(state, body),
        "/api/iota/uninstall" => iota_name_op(state, body, |m, n| m.uninstall(n)),
        "/api/iota/install_url" => {
            if method != "POST" {
                err("需要 POST")
            } else {
                let fields = json::parse_form(body);
                let url = json::form_get(&fields, "url").unwrap_or("").trim().to_string();
                let sha = json::form_get(&fields, "sha256").unwrap_or("").trim().to_string();
                if url.is_empty() {
                    err("缺少 url")
                } else {
                    ok_bool2(state.iota.install_url(&url, &sha))
                }
            }
        }
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

/// iota 按名字的单参数操作（start/stop/restart/uninstall）。
fn iota_name_op<F>(state: &crate::http::State, body: &[u8], f: F) -> String
where
    F: Fn(&crate::iota::Manager, &str) -> (bool, String),
{
    let fields = json::parse_form(body);
    let name = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
    if name.is_empty() {
        return err("缺少 name");
    }
    let (ok, msg) = f(&state.iota, &name);
    ok_bool(ok, msg)
}

/// iota 保活开关。
fn iota_keepalive_op(state: &crate::http::State, body: &[u8]) -> String {
    let fields = json::parse_form(body);
    let name = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
    if name.is_empty() {
        return err("缺少 name");
    }
    let on = json::form_get(&fields, "on").unwrap_or("0").trim() == "1"
        || json::form_get(&fields, "on").unwrap_or("0").trim() == "true";
    let (ok, msg) = state.iota.set_keepalive(&name, on);
    ok_bool(ok, msg)
}

/// `(bool, String)` -> 结果 JSON（与 dbj 等效）。
fn ok_bool2(r: (bool, String)) -> String {
    ok_bool(r.0, r.1)
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
        "/api/nginx/add" => {
            let fname = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let server_name = json::form_get(&fields, "server_name").unwrap_or("").trim().to_string();
            let listen = json::form_get(&fields, "listen").unwrap_or("80").trim().to_string();
            let target = json::form_get(&fields, "target").unwrap_or("").trim().to_string();
            let (ok, msg) = crate::nginx::nginx_add(&fname, &server_name, &listen, &target);
            ok_bool(ok, msg)
        }
        "/api/nginx/toggle" => {
            let fname = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let enable = json::form_get(&fields, "enable").unwrap_or("").trim() == "true" || json::form_get(&fields, "enable").unwrap_or("").trim() == "1";
            let (ok, msg) = crate::nginx::nginx_toggle(&fname, enable);
            ok_bool(ok, msg)
        }
        "/api/nginx/delete" => {
            let fname = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let (ok, msg) = crate::nginx::nginx_delete(&fname);
            ok_bool(ok, msg)
        }
        "/api/nginx/reload" => {
            let (ok, msg) = crate::nginx::nginx_reload_endpoint();
            ok_bool(ok, msg)
        }
        "/api/website/create" => {
            let name = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let domain = json::form_get(&fields, "domain").unwrap_or("").trim().to_string();
            let listen = json::form_get(&fields, "listen").unwrap_or("80").trim().to_string();
            let php = json::form_get(&fields, "php").unwrap_or("0").trim() == "1"
                || json::form_get(&fields, "php").unwrap_or("0").trim() == "true";
            let phpver = json::form_get(&fields, "php_version").unwrap_or("").trim().to_string();
            let (ok, msg) = crate::website::website_create(&name, &domain, &listen, php, &phpver);
            ok_bool(ok, msg)
        }
        "/api/website/toggle" => {
            let name = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let enable = json::form_get(&fields, "enable").unwrap_or("").trim() == "true"
                || json::form_get(&fields, "enable").unwrap_or("0").trim() == "1";
            let (ok, msg) = crate::website::website_toggle(&name, enable);
            ok_bool(ok, msg)
        }
        "/api/website/delete" => {
            let name = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let drop_root = json::form_get(&fields, "drop_root").unwrap_or("0").trim() == "1"
                || json::form_get(&fields, "drop_root").unwrap_or("0").trim() == "true";
            let (ok, msg) = crate::website::website_delete(&name, drop_root);
            ok_bool(ok, msg)
        }
        "/api/website/rewrite" => {
            let name = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let kind = json::form_get(&fields, "kind").unwrap_or("none").trim().to_string();
            let (ok, msg) = crate::website::rewrite_apply(&name, &kind);
            ok_bool(ok, msg)
        }
        "/api/autostart" => {
            let name = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let enable = json::form_get(&fields, "enable").unwrap_or("").trim() == "true" || json::form_get(&fields, "enable").unwrap_or("").trim() == "1";
            if name.is_empty() {
                return err("缺少 name");
            }
            let (ok, msg) = crate::nginx::autostart_action(&name, enable);
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
        "/api/db/create_db" => {
            let name = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let charset = json::form_get(&fields, "charset").unwrap_or("").trim().to_string();
            dbj(crate::db::create_db(&state_cfg_database(), &name, &charset))
        }
        "/api/db/drop_db" => {
            let name = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            dbj(crate::db::drop_db(&state_cfg_database(), &name))
        }
        "/api/db/create_user" => {
            let u = json::form_get(&fields, "user").unwrap_or("").trim().to_string();
            let p = json::form_get(&fields, "pass").unwrap_or("").to_string();
            let h = json::form_get(&fields, "host").unwrap_or("").trim().to_string();
            dbj(crate::db::create_user(&state_cfg_database(), &u, &p, &h))
        }
        "/api/db/drop_user" => {
            let u = json::form_get(&fields, "user").unwrap_or("").trim().to_string();
            let h = json::form_get(&fields, "host").unwrap_or("").trim().to_string();
            dbj(crate::db::drop_user(&state_cfg_database(), &u, &h))
        }
        "/api/db/grant" => {
            let d = json::form_get(&fields, "db").unwrap_or("").trim().to_string();
            let u = json::form_get(&fields, "user").unwrap_or("").trim().to_string();
            let h = json::form_get(&fields, "host").unwrap_or("").trim().to_string();
            dbj(crate::db::grant(&state_cfg_database(), &d, &u, &h))
        }
        "/api/db/backup" => {
            let d = json::form_get(&fields, "db").unwrap_or("").trim().to_string();
            let dir = &state_cfg_database().backup_dir;
            dbj(crate::db::backup(&state_cfg_database(), &d, dir))
        }
        "/api/db/restore" => {
            let d = json::form_get(&fields, "db").unwrap_or("").trim().to_string();
            let f = json::form_get(&fields, "file").unwrap_or("").trim().to_string();
            dbj(crate::db::restore(&state_cfg_database(), &d, &f))
        }
        "/api/db/reset_root" => {
            let npw = json::form_get(&fields, "password").unwrap_or("").trim().to_string();
            dbj(crate::db::reset_root_password(&state_cfg_database(), &npw))
        }
        "/api/ssl/import" => {
            let n = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let fc = json::form_get(&fields, "fullchain").unwrap_or("").to_string();
            let pk = json::form_get(&fields, "privkey").unwrap_or("").to_string();
            dbj(crate::ssl::import(&state_certs(), &n, &fc, &pk))
        }
        "/api/ssl/self_signed" => {
            let n = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let d = json::form_get(&fields, "domain").unwrap_or("").trim().to_string();
            let days: u32 = json::form_get(&fields, "days").and_then(|x| x.parse().ok()).unwrap_or(365);
            dbj(crate::ssl::self_signed(&state_certs(), &n, &d, days))
        }
        "/api/ssl/le_issue" => {
            let n = json::form_get(&fields, "name").unwrap_or("").trim().to_string();
            let d = json::form_get(&fields, "domain").unwrap_or("").trim().to_string();
            let wr = json::form_get(&fields, "webroot").unwrap_or("").trim().to_string();
            dbj(crate::ssl::le_issue(&state_certs(), &n, &d, &wr))
        }
        "/api/ssl/apply" => {
            let site = json::form_get(&fields, "site").unwrap_or("").trim().to_string();
            let cname = json::form_get(&fields, "cert").unwrap_or("").trim().to_string();
            let upgrade = json::form_get(&fields, "upgrade").unwrap_or("0").trim()
                == "1"
                || json::form_get(&fields, "upgrade").unwrap_or("0").trim() == "true";
            dbj(crate::ssl::apply(&state_certs(), &site, &cname, upgrade))
        }
        "/api/security/ban" => {
            let ip = json::form_get(&fields, "ip").unwrap_or("").trim().to_string();
            dbj(crate::security::ban_ip(&ip))
        }
        "/api/security/unban" => {
            let ip = json::form_get(&fields, "ip").unwrap_or("").trim().to_string();
            dbj(crate::security::unban_ip(&ip))
        }
        "/api/security/brute" => {
            let t: u32 = json::form_get(&fields, "threshold").and_then(|x| x.parse().ok()).unwrap_or(5);
            String::from(crate::security::brute_scan(t))
        }
        "/api/security/waf/enable" => {
            let rps: u32 = json::form_get(&fields, "rps").and_then(|x| x.parse().ok()).unwrap_or(20);
            let burst: u32 = json::form_get(&fields, "burst").and_then(|x| x.parse().ok()).unwrap_or(40);
            dbj(crate::security::waf_apply(rps, burst))
        }
        "/api/security/waf/disable" => dbj(crate::security::waf_disable()),
        "/api/security/harden" => {
            let nopass = json::form_get(&fields, "no_root_pass").unwrap_or("0").trim() == "1"
                || json::form_get(&fields, "no_root_pass").unwrap_or("0").trim() == "true";
            let nopw = json::form_get(&fields, "no_password").unwrap_or("0").trim() == "1"
                || json::form_get(&fields, "no_password").unwrap_or("0").trim() == "true";
            dbj(crate::security::harden_ssh(nopass, nopw))
        }
        "/api/security/unharden" => dbj(crate::security::unharden_ssh()),
        "/api/backup/dir" => {
            let src = json::form_get(&fields, "path").unwrap_or("").trim().to_string();
            let keep: u32 = json::form_get(&fields, "keep").and_then(|x| x.parse().ok()).unwrap_or(5);
            let (ok, msg) = crate::backup::dir_backup(&src, &state_cfg_backup().dir, keep);
            dbj((ok, msg))
        }
        "/api/backup/run" => {
            let (ok, msg) = crate::backup::run(&state_cfg_backup_full());
            dbj((ok, msg))
        }
        "/api/backup/schedule" => {
            let cron = json::form_get(&fields, "cron").unwrap_or("").trim().to_string();
            let (ok, msg) = crate::backup::schedule(&state_cfg_backup_full(), &cron);
            dbj((ok, msg))
        }
        "/api/backup/schedule_remove" => dbj(crate::backup::schedule_remove()),
        "/api/backup/cloud" => {
            let file = json::form_get(&fields, "file").unwrap_or("").trim().to_string();
            dbj(crate::backup::cloud_upload(&file))
        }
        "/api/env/install" => {
            let id = json::form_get(&fields, "id").unwrap_or("").trim().to_string();
            dbj(crate::env::install(&id))
        }
        "/api/env/service" => {
            let id = json::form_get(&fields, "id").unwrap_or("").trim().to_string();
            let action = json::form_get(&fields, "action").unwrap_or("").trim().to_string();
            dbj(crate::env::service(&id, &action))
        }
        _ => err("未知操作"),
    }
}

// 下面三个辅助：action_route 拿不到 State，只能依赖常量/线程本地拿到数据库配置。
// 由于 action_route 无 cfg 上下文，把数据库配置预置到全局（服务启动时注入一次）。
use std::sync::OnceLock;

fn state_cfg_database() -> &'static crate::config::Database {
    STATE_DB.get_or_init(|| crate::config::Database::default())
}

static STATE_DB: OnceLock<crate::config::Database> = OnceLock::new();

/// 服务启动时把配置里的数据库段注入全局，供无 State 的 action_route 使用。
pub fn db_config_init(cfg: &crate::config::Config) {
    let _ = STATE_DB.set(cfg.database.clone());
}

fn state_certs() -> &'static crate::config::Certs {
    STATE_CERTS.get_or_init(|| crate::config::Certs::default())
}

static STATE_CERTS: OnceLock<crate::config::Certs> = OnceLock::new();

/// 注入证书配置。
pub fn certs_config_init(cfg: &crate::config::Config) {
    let _ = STATE_CERTS.set(cfg.certs.clone());
}

fn state_cfg_backup() -> &'static crate::config::Backup {
    STATE_BACKUP.get_or_init(|| crate::config::Backup::default())
}

static STATE_BACKUP: OnceLock<crate::config::Backup> = OnceLock::new();

/// 注入备份配置。
pub fn backup_config_init(cfg: &crate::config::Config) {
    let _ = STATE_BACKUP.set(cfg.backup.clone());
}

fn state_cfg_backup_full() -> &'static crate::config::Config {
    STATE_CFG.get_or_init(crate::config::Config::default)
}

static STATE_CFG: OnceLock<crate::config::Config> = OnceLock::new();

/// 注入完整配置（供 run/schedule 使用 database + backup）。
pub fn config_init(cfg: &crate::config::Config) {
    let _ = STATE_CFG.set(cfg.clone());
}

/// `(bool, String)` -> 结果 JSON。
fn dbj(r: (bool, String)) -> String {
    if r.0 {
        format!("{{\"ok\":true,\"msg\":\"{}\"}}", json::jesc(&r.1))
    } else {
        format!("{{\"ok\":false,\"msg\":\"{}\"}}", json::jesc(&r.1))
    }
}

/// 数据库列表类：成功时第二项已是 JSON 数组字符串。
fn dbj2(r: (bool, String)) -> String {
    if r.0 {
        format!("{{\"ok\":true,\"data\":{}}}", r.1)
    } else {
        format!("{{\"ok\":false,\"msg\":\"{}\"}}", json::jesc(&r.1))
    }
}

/// 数据库安装/运行状态。
fn db_status_json(cfg: &crate::config::Config) -> String {
    let installed = crate::db::installed(&cfg.database);
    let running = installed && crate::db::server_running(&cfg.database);
    format!(
        "{{\"ok\":true,\"installed\":{},\"running\":{},\"user\":\"{}\"}}",
        installed,
        running,
        json::jesc(&cfg.database.user)
    )
}

/// 列出数据库备份文件。
fn db_backups_json(cfg: &crate::config::Config) -> String {
    let dir = &cfg.database.backup_dir;
    let mut files: Vec<(String, u64, u64)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().extension().map_or(false, |x| x == "gz") {
                let name = e.file_name().to_string_lossy().into_owned();
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                let mtime = e.metadata().and_then(|m| m.modified()).ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                files.push((name, size, mtime));
            }
        }
    }
    files.sort_by(|a, b| b.2.cmp(&a.2));
    let arr = files
        .iter()
        .map(|(n, s, t)| format!("{{\"name\":\"{}\",\"size\":{},\"mtime\":{}}}", json::jesc(n), s, t))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"ok\":true,\"list\":[{}],\"dir\":\"{}\"}}", arr, json::jesc(dir))
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