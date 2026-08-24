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
    let result = "{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"vpanel\",\"version\":\"1.4.0\"}}";
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
        ("nginx_reload","重载 Nginx 配置","{}"),
        ("autostart","设置站点/服务开机自启，参数 name 与 enable(布尔)","{\"name\":{\"type\":\"string\"},\"enable\":{\"type\":\"boolean\"}}"),
        // ---- 网站（对标宝塔「网站」）----
        ("website_list","列出网站（域名/端口/根目录/PHP标记/启用状态）","{}"),
        ("website_create","创建网站，参数 name/domain/listen/php(布尔)/php_version","{\"name\":{\"type\":\"string\"},\"domain\":{\"type\":\"string\"},\"listen\":{\"type\":\"string\"},\"php\":{\"type\":\"boolean\"},\"php_version\":{\"type\":\"string\"}}"),
        ("website_toggle","启用/停用网站，参数 name 与 enable(布尔)","{\"name\":{\"type\":\"string\"},\"enable\":{\"type\":\"boolean\"}}"),
        ("website_delete","删除网站，参数 name 与 drop_root(布尔，是否连根目录一起删)","{\"name\":{\"type\":\"string\"},\"drop_root\":{\"type\":\"boolean\"}}"),
        ("website_rewrite","为站点应用伪静态规则，参数 name 与 kind(wordpress/thinkphp/laravel/none)","{\"name\":{\"type\":\"string\"},\"kind\":{\"type\":\"string\"}}"),
        // ---- 数据库（对标宝塔「数据库」）----
        ("db_status","查看数据库安装与运行状态","{}"),
        ("db_databases","列出所有数据库（排除系统库）","{}"),
        ("db_users","列出所有数据库账号（user/host）","{}"),
        ("db_backups","列出数据库备份文件","{}"),
        ("db_create_db","创建数据库，参数 name 与 charset(默认utf8mb4)","{\"name\":{\"type\":\"string\"},\"charset\":{\"type\":\"string\"}}"),
        ("db_drop_db","删除数据库，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("db_create_user","创建数据库账号，参数 user/pass/host","{\"user\":{\"type\":\"string\"},\"pass\":{\"type\":\"string\"},\"host\":{\"type\":\"string\"}}"),
        ("db_drop_user","删除数据库账号，参数 user/host","{\"user\":{\"type\":\"string\"},\"host\":{\"type\":\"string\"}}"),
        ("db_grant","将某库权限授予账号，参数 db/user/host","{\"db\":{\"type\":\"string\"},\"user\":{\"type\":\"string\"},\"host\":{\"type\":\"string\"}}"),
        ("db_backup","备份指定数据库，参数 db","{\"db\":{\"type\":\"string\"}}"),
        ("db_restore","从备份文件恢复数据库，参数 db/file","{\"db\":{\"type\":\"string\"},\"file\":{\"type\":\"string\"}}"),
        ("db_reset_root","重置 MySQL root 密码，参数 password","{\"password\":{\"type\":\"string\"}}"),
        // ---- SSL 证书（对标宝塔「SSL」）----
        ("ssl_list","列出 SSL 证书","{}"),
        ("ssl_import","导入证书，参数 name/fullchain/privkey","{\"name\":{\"type\":\"string\"},\"fullchain\":{\"type\":\"string\"},\"privkey\":{\"type\":\"string\"}}"),
        ("ssl_self_signed","生成自签证书，参数 name/domain/days","{\"name\":{\"type\":\"string\"},\"domain\":{\"type\":\"string\"},\"days\":{\"type\":\"number\"}}"),
        ("ssl_le_issue","申请 Let's Encrypt 证书，参数 name/domain/webroot","{\"name\":{\"type\":\"string\"},\"domain\":{\"type\":\"string\"},\"webroot\":{\"type\":\"string\"}}"),
        ("ssl_apply","为站点应用证书，参数 site/cert/upgrade(布尔)","{\"site\":{\"type\":\"string\"},\"cert\":{\"type\":\"string\"},\"upgrade\":{\"type\":\"boolean\"}}"),
        // ---- 运行环境（对标宝塔「软件商店/环境」）----
        ("env_status","查看运行环境总览（nginx/mysql/redis/php/node/docker/python/go）","{}"),
        ("env_install","一键安装运行时，参数 id(nginx/mysql/redis/php/node/docker/go)","{\"id\":{\"type\":\"string\"}}"),
        ("env_service","启停运行时服务，参数 id 与 action(start/stop/restart)","{\"id\":{\"type\":\"string\"},\"action\":{\"type\":\"string\"}}"),
        // ---- 备份（对标宝塔「定时备份」）----
        ("backup_list","列出备份文件","{}"),
        ("backup_dir","备份目录，参数 path 与 keep(保留份数)","{\"path\":{\"type\":\"string\"},\"keep\":{\"type\":\"number\"}}"),
        ("backup_run","执行全量备份（目录+数据库）","{}"),
        ("backup_schedule","设置定时备份，参数 cron(5段cron)","{\"cron\":{\"type\":\"string\"}}"),
        ("backup_schedule_remove","移除定时备份","{}"),
        ("backup_cloud","上传备份到云存储(FTP)，参数 file","{\"file\":{\"type\":\"string\"}}"),
        // ---- 安全（对标宝塔付费「安全/WAF」）----
        ("security_bans","列出已封禁 IP","{}"),
        ("security_hardening","查看系统加固状态（SSH）","{}"),
        ("security_waf_status","查看 WAF 状态","{}"),
        ("security_ban","封禁 IP，参数 ip","{\"ip\":{\"type\":\"string\"}}"),
        ("security_unban","解封 IP，参数 ip","{\"ip\":{\"type\":\"string\"}}"),
        ("security_brute","扫描暴力破解并自动封禁，参数 threshold(默认5)","{\"threshold\":{\"type\":\"number\"}}"),
        ("security_waf_enable","启用 WAF，参数 rps/burst","{\"rps\":{\"type\":\"number\"},\"burst\":{\"type\":\"number\"}}"),
        ("security_waf_disable","关闭 WAF","{}"),
        ("security_harden","SSH 加固，参数 no_root_pass/no_password(布尔)","{\"no_root_pass\":{\"type\":\"boolean\"},\"no_password\":{\"type\":\"boolean\"}}"),
        ("security_unharden","撤销 SSH 加固","{}"),
        // ---- IotaPanel 兼容插件（独立进程插件）----
        ("iota_list","列出 IotaPanel 兼容插件","{}"),
        ("iota_status","查看插件运行状态，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("iota_log","查看插件日志，参数 name 与 n(行数)","{\"name\":{\"type\":\"string\"},\"n\":{\"type\":\"number\"}}"),
        ("iota_start","启动插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("iota_stop","停止插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("iota_restart","重启插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("iota_uninstall","卸载插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("iota_keepalive","设置插件保活，参数 name 与 on(布尔)","{\"name\":{\"type\":\"string\"},\"on\":{\"type\":\"boolean\"}}"),
        ("iota_install_url","从 URL 安装插件，参数 url 与 sha256(可选)","{\"url\":{\"type\":\"string\"},\"sha256\":{\"type\":\"string\"}}"),
        // ---- 插件商店 & KV ----
        ("plugin_store","列出插件商店软件","{}"),
        ("plugin_store_install","从商店安装插件，参数 id","{\"id\":{\"type\":\"string\"}}"),
        ("plugin_kv","列出插件 KV 存储","{}"),
        ("plugin_enable","启用插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("plugin_disable","禁用插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        // ---- 监控 / 资源排行 ----
        ("resource_top","资源占用排行（按 CPU/内存前 n），参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("monitor_snapshot","查看历史监控数据（最近 n 个采样点），参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("shop_list","列出软件商店应用","{}"),
        ("log_follow","增量查看日志，参数 file 与 pos(字节偏移)","{\"file\":{\"type\":\"string\"},\"pos\":{\"type\":\"number\"}}"),
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
    let pos: u64 = arg_str(&args, "pos").parse().unwrap_or(0);
    let server_name = arg_str(&args, "server_name");
    let listen = arg_str(&args, "listen");
    let target = arg_str(&args, "target");
    let fname = arg_str(&args, "name");
    let enable = arg_str(&args, "enable") == "true" || arg_str(&args, "enable") == "1";
    // 网站
    let domain = arg_str(&args, "domain");
    let php = arg_str(&args, "php") == "true" || arg_str(&args, "php") == "1";
    let php_version = arg_str(&args, "php_version");
    let drop_root = arg_str(&args, "drop_root") == "true" || arg_str(&args, "drop_root") == "1";
    let kind = arg_str(&args, "kind");
    // 数据库
    let dbname = arg_str(&args, "name");
    let charset = arg_str(&args, "charset");
    let dbuser = arg_str(&args, "user");
    let dbpass = arg_str(&args, "pass");
    let host = arg_str(&args, "host");
    let db = arg_str(&args, "db");
    let password = arg_str(&args, "password");
    // SSL
    let fullchain = arg_str(&args, "fullchain");
    let privkey = arg_str(&args, "privkey");
    let cert = arg_str(&args, "cert");
    let site = arg_str(&args, "site");
    let days: u32 = arg_str(&args, "days").parse().unwrap_or(365);
    let webroot = arg_str(&args, "webroot");
    let upgrade = arg_str(&args, "upgrade") == "true" || arg_str(&args, "upgrade") == "1";
    // 环境 / 备份 / 安全
    let env_id = arg_str(&args, "id");
    let cron = arg_str(&args, "cron");
    // cron 与 file 已复用上面 command/file 变量？file 单独解析：
    // 备份目录 path 复用 path；keep
    let keep: u32 = arg_str(&args, "keep").parse().unwrap_or(5);
    // 安全
    let ip = arg_str(&args, "ip");
    let threshold: u32 = arg_str(&args, "threshold").parse().unwrap_or(5);
    let rps: u32 = arg_str(&args, "rps").parse().unwrap_or(20);
    let burst: u32 = arg_str(&args, "burst").parse().unwrap_or(40);
    let no_root_pass = arg_str(&args, "no_root_pass") == "true" || arg_str(&args, "no_root_pass") == "1";
    let no_password = arg_str(&args, "no_password") == "true" || arg_str(&args, "no_password") == "1";
    // iota
    let url = arg_str(&args, "url");
    let sha256 = arg_str(&args, "sha256");

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
        "nginx_reload" => crate::nginx::nginx_reload_endpoint(),
        "autostart" => crate::nginx::autostart_action(&fname, enable),
        // ---- 网站 ----
        "website_list" => (true, crate::website::website_list_json()),
        "website_create" => {
            let (o, m) = crate::website::website_create(&fname, &domain, &listen, php, &php_version);
            (o, m)
        }
        "website_toggle" => {
            let (o, m) = crate::website::website_toggle(&fname, enable);
            (o, m)
        }
        "website_delete" => {
            let (o, m) = crate::website::website_delete(&fname, drop_root);
            (o, m)
        }
        "website_rewrite" => {
            let (o, m) = crate::website::rewrite_apply(&fname, &kind);
            (o, m)
        }
        // ---- 数据库 ----
        "db_status" => {
            let installed = crate::db::installed(&state.cfg.database);
            let running = installed && crate::db::server_running(&state.cfg.database);
            (true, format!(
                "{{\"ok\":true,\"installed\":{},\"running\":{},\"user\":\"{}\"}}",
                installed, running, json::jesc(&state.cfg.database.user)
            ))
        }
        "db_databases" => {
            let (o, m) = crate::db::databases(&state.cfg.database);
            (o, format!("{{\"ok\":{},\"list\":{}}}", o, m))
        }
        "db_users" => {
            let (o, m) = crate::db::users(&state.cfg.database);
            (o, format!("{{\"ok\":{},\"list\":{}}}", o, m))
        }
        "db_backups" => (true, db_backups_text(&state.cfg)),
        "db_create_db" => crate::db::create_db(&state.cfg.database, &dbname, &charset),
        "db_drop_db" => crate::db::drop_db(&state.cfg.database, &dbname),
        "db_create_user" => crate::db::create_user(&state.cfg.database, &dbuser, &dbpass, &host),
        "db_drop_user" => crate::db::drop_user(&state.cfg.database, &dbuser, &host),
        "db_grant" => crate::db::grant(&state.cfg.database, &db, &dbuser, &host),
        "db_backup" => crate::db::backup(&state.cfg.database, &db, &state.cfg.database.backup_dir),
        "db_restore" => crate::db::restore(&state.cfg.database, &db, &file),
        "db_reset_root" => crate::db::reset_root_password(&state.cfg.database, &password),
        // ---- SSL ----
        "ssl_list" => (true, crate::ssl::list_json(&state.cfg.certs)),
        "ssl_import" => crate::ssl::import(&state.cfg.certs, &fname, &fullchain, &privkey),
        "ssl_self_signed" => crate::ssl::self_signed(&state.cfg.certs, &fname, &domain, days),
        "ssl_le_issue" => crate::ssl::le_issue(&state.cfg.certs, &fname, &domain, &webroot),
        "ssl_apply" => crate::ssl::apply(&state.cfg.certs, &site, &cert, upgrade),
        // ---- 运行环境 ----
        "env_status" => (true, crate::env::status_json()),
        "env_install" => crate::env::install(&env_id),
        "env_service" => crate::env::service(&env_id, &action),
        // ---- 备份 ----
        "backup_list" => (true, crate::backup::list_json(&state.cfg)),
        "backup_dir" => crate::backup::dir_backup(&path, &state.cfg.backup.dir, keep),
        "backup_run" => crate::backup::run(&state.cfg),
        "backup_schedule" => crate::backup::schedule(&state.cfg, &cron),
        "backup_schedule_remove" => crate::backup::schedule_remove(),
        "backup_cloud" => crate::backup::cloud_upload(&file),
        // ---- 安全 ----
        "security_bans" => (true, crate::security::bans_json()),
        "security_hardening" => (true, crate::security::hardening_status()),
        "security_waf_status" => (true, crate::extra::waf_status_json(&state.cfg)),
        "security_ban" => crate::security::ban_ip(&ip),
        "security_unban" => crate::security::unban_ip(&ip),
        "security_brute" => (true, crate::security::brute_scan(threshold)),
        "security_waf_enable" => crate::security::waf_apply(rps, burst),
        "security_waf_disable" => crate::security::waf_disable(),
        "security_harden" => crate::security::harden_ssh(no_root_pass, no_password),
        "security_unharden" => crate::security::unharden_ssh(),
        // ---- IotaPanel 兼容插件 ----
        "iota_list" => (true, state.iota.list_json()),
        "iota_status" => (true, state.iota.status_json(&fname)),
        "iota_log" => (true, state.iota.log_tail_json(&fname, n)),
        "iota_start" => match state.iota.start(&fname) {
            Ok((_, p)) => (true, format!("插件 {} 已启动（端口 {}）", fname, p)),
            Err(e) => (false, e),
        },
        "iota_stop" => state.iota.stop(&fname),
        "iota_restart" => state.iota.restart(&fname),
        "iota_uninstall" => state.iota.uninstall(&fname),
        "iota_keepalive" => state.iota.set_keepalive(&fname, enable),
        "iota_install_url" => state.iota.install_url(&url, &sha256),
        // ---- 插件商店 / KV / 启停 ----
        "plugin_store" => (true, state.plugins.store_list_json(&state.cfg)),
        "plugin_store_install" => state.plugins.store_install(&env_id, &state.cfg),
        "plugin_kv" => (true, state.plugins.kv_list_json()),
        "plugin_enable" => state.plugins.set_enabled(&fname, true),
        "plugin_disable" => state.plugins.set_enabled(&fname, false),
        // ---- 监控 / 资源排行 ----
        "resource_top" => (true, crate::extra::resources_top_json(n)),
        "monitor_snapshot" => (true, crate::monitor::monitor_json(n)),
        "shop_list" => (true, state.shop.list_json(&state.cfg)),
        "log_follow" => (true, crate::extra::log_follow_json(&file, pos)),
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

/// 列出数据库备份文件（对标 api.rs 的 db_backups_json）。
fn db_backups_text(cfg: &crate::config::Config) -> String {
    let dir = &cfg.database.backup_dir;
    let mut files: Vec<(String, u64)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().extension().map_or(false, |x| x == "gz") {
                let name = e.file_name().to_string_lossy().into_owned();
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                files.push((name, size));
            }
        }
    }
    files.sort();
    let arr = files
        .iter()
        .map(|(n, s)| format!("{{\"name\":\"{}\",\"size\":{}}}", json::jesc(n), s))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"ok\":true,\"dir\":\"{}\",\"list\":[{}]}}", json::jesc(dir), arr)
}