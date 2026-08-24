//! Nginx 反向代理 / 站点管理。
//!
//! 基于 /etc/nginx 的 sites-available + sites-enabled（符号链接启用）模型：
//! 列出/新增/启停/删除站点，改动后执行 `nginx -t` 校验并 `nginx -s reload`。
//! 所有操作按需执行为一次性子进程，无常驻状态。

use crate::json;

/// nginx 配置根目录（可被 panel.yml 的 nginx.dir 覆盖）。
pub fn conf_dir() -> String {
    std::env::var("VPANEL_NGINX_DIR")
        .unwrap_or_else(|_| "/etc/nginx".to_string())
}

fn avail_dir() -> String {
    format!("{}/sites-available", conf_dir())
}

fn enabled_dir() -> String {
    format!("{}/sites-enabled", conf_dir())
}

fn conf_ext() -> &'static str {
    ".conf"
}

fn available() -> bool {
    std::path::Path::new(&avail_dir()).is_dir()
}

fn run(cmd: &str) -> (bool, String) {
    let out = std::process::Command::new("/bin/sh").arg("-c").arg(cmd).output();
    match out {
        Ok(o) => {
            let msg = {
                let s = String::from_utf8_lossy(&o.stdout);
                let e = String::from_utf8_lossy(&o.stderr);
                format!("{}{}", s, e).trim().to_string()
            };
            (o.status.success(), msg)
        }
        Err(e) => (false, e.to_string()),
    }
}

/// 校验 nginx 配置。（跨模块复用：security WAF / ssl 应用）
pub fn nginx_test() -> (bool, String) {
    run("nginx -t 2>&1")
}

pub fn nginx_reload() -> (bool, String) {
    run("nginx -s reload 2>&1 || systemctl reload nginx 2>&1")
}

/// 站点列表 -> JSON。
pub fn nginx_list_json() -> String {
    if !available() {
        return "{\"ok\":false,\"msg\":\"未找到 nginx 配置目录 ".to_string()
            + &json::jesc(&avail_dir())
            + "\"}";
    }
    let mut items = Vec::new();
    if let Ok(rd) = std::fs::read_dir(avail_dir()) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            if !name.ends_with(conf_ext()) {
                continue;
            }
            let base = name.trim_end_matches(conf_ext()).to_string();
            let enabled = std::path::Path::new(&enabled_dir()).join(&name).exists();
            let content = std::fs::read_to_string(ent.path()).unwrap_or_default();
            let (listen, server_name, proxy_pass, ssl) = parse_conf(&content);
            items.push(format!(
                "{{\"name\":\"{}\",\"enabled\":{},\"listen\":\"{}\",\"server_name\":\"{}\",\"proxy_pass\":\"{}\",\"ssl\":{}}}",
                json::jesc(&base),
                enabled,
                json::jesc(&listen),
                json::jesc(&server_name),
                json::jesc(&proxy_pass),
                ssl
            ));
        }
    }
    items.sort();
    format!("{{\"ok\":true,\"basedir\":\"{}\",\"list\":[{}]}}", json::jesc(&conf_dir()), items.join(","))
}

fn parse_conf(content: &str) -> (String, String, String, bool) {
    let mut listen = String::new();
    let mut server_name = String::new();
    let mut proxy_pass = String::new();
    let mut ssl = false;
    for line in content.lines() {
        let t = line.trim();
        if let Some(v) = t.find("server_name") {
            if let Some(rest) = t.get(v + "server_name".len()..) {
                let first = rest.split_whitespace().next().unwrap_or("");
                // 去掉末尾分号
                server_name = first.trim_end_matches(';').to_string();
            }
        } else if let Some(v) = t.find("listen") {
            if let Some(rest) = t.get(v + "listen".len()..) {
                let first = rest.split_whitespace().next().unwrap_or("");
                listen = first.trim_end_matches(';').to_string();
            }
        } else if let Some(v) = t.find("proxy_pass ") {
            if let Some(rest) = t.get(v + "proxy_pass ".len()..) {
                proxy_pass = rest.trim_end_matches(';').trim().to_string();
            }
        } else if t.starts_with("ssl_certificate ") {
            ssl = true;
        }
        if !server_name.is_empty() && !listen.is_empty() && !proxy_pass.is_empty() && ssl {
            break;
        }
    }
    (listen, server_name, proxy_pass, ssl)
}

/// 校验站点命名合法（字母数字、连字符、点、下划线）。
fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 64
        && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
}

/// 新增反向代理站点。
pub fn nginx_add(name: &str, server_name: &str, listen: &str, target: &str) -> (bool, String) {
    if !available() {
        return (false, format!("未找到 nginx 配置目录 {}", avail_dir()));
    }
    if !valid_name(name) {
        return (false, "站点名只能含字母/数字/连字符/点/下划线".into());
    }
    let listen = listen.trim();
    if listen.is_empty() || !listen.chars().all(|c| c.is_ascii_digit()) {
        return (false, "listen 需为端口数字".into());
    }
    if server_name.trim().is_empty() {
        return (false, "缺少 server_name（域名）".into());
    }
    let target = target.trim();
    if !target.starts_with("http://") && !target.starts_with("https://") {
        return (false, "反向代理目标需以 http:// 或 https:// 开头".into());
    }
    let conf = reverse_proxy_conf(server_name.trim(), &listen, target);
    let file = format!("{}/{}.conf", avail_dir(), name);
    if std::fs::write(&file, conf.as_bytes()).is_err() {
        return (false, format!("写配置文件失败：{}", file));
    }
    let (ok, msg) = nginx_test();
    if !ok {
        let _ = std::fs::remove_file(&file);
        return (false, format!("nginx 配置校验失败（已回滚）：\n{}", msg));
    }
    let lnk = format!("{}/{}.conf", enabled_dir(), name);
    if !std::path::Path::new(&lnk).exists() {
        let _ = std::process::Command::new("ln")
            .args(["-s", &format!("{}.conf", name), &lnk])
            .status();
    }
    let (ro, rm) = nginx_reload();
    if ro {
        (true, format!("站点 {} 已创建并启用（代理 -> {}）", name, target))
    } else {
        (false, format!("配置已写入但 reload 失败：{}", rm))
    }
}

fn reverse_proxy_conf(server_name: &str, listen: &str, target: &str) -> String {
    let t = r#"# auto-generated by vPanel
server {
    listen ###LISTEN###;
    server_name ###SERVER_NAME###;

    location / {
        proxy_pass ###TARGET###;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
"#;
    t.replace("###LISTEN###", listen)
        .replace("###SERVER_NAME###", server_name)
        .replace("###TARGET###", target)
}

/// 给站点启用 HTTPS：读出现有 server_name / proxy_pass，重写为
/// 「80 -> 301 跳 https」+「443 ssl」双 server 块，指向传入的证书。
/// `upgrade` 为真时把 80 也收编为跳转；证书文件需已存在。
pub fn nginx_ssl(name: &str, cert: &str, key: &str, upgrade: bool) -> (bool, String) {
    if !available() {
        return (false, format!("未找到 nginx 配置目录 {}", avail_dir()));
    }
    if !valid_name(name) {
        return (false, "非法的站点名".into());
    }
    if !std::path::Path::new(cert).is_file() || !std::path::Path::new(key).is_file() {
        return (false, "证书或私钥文件不存在".into());
    }
    let file = format!("{}/{}.conf", avail_dir(), name);
    let content = std::fs::read_to_string(&file).unwrap_or_default();
    let (_, server_name, proxy_pass, _) = parse_conf(&content);
    if server_name.is_empty() || proxy_pass.is_empty() {
        return (false, "读取站点 server_name / 反代目标失败".into());
    }
    let mut ssl_block = String::new();
    ssl_block.push_str("server {\n");
    ssl_block.push_str("    listen 443 ssl;\n");
    ssl_block.push_str("    http2 on;\n");
    ssl_block.push_str(&format!("    server_name {};\n", server_name));
    ssl_block.push_str("    ssl_protocols TLSv1.2 TLSv1.3;\n");
    ssl_block.push_str("    ssl_ciphers HIGH:!aNULL:!MD5;\n");
    ssl_block.push_str(&format!("    ssl_certificate {};\n", cert));
    ssl_block.push_str(&format!("    ssl_certificate_key {};\n", key));
    ssl_block.push_str("    location / {\n");
    ssl_block.push_str(&format!("        proxy_pass {};\n", proxy_pass));
    ssl_block.push_str("        proxy_set_header Host $host;\n");
    ssl_block.push_str("        proxy_set_header X-Real-IP $remote_addr;\n");
    ssl_block.push_str("        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n");
    ssl_block.push_str("        proxy_set_header X-Forwarded-Proto $scheme;\n");
    ssl_block.push_str("    }\n");
    ssl_block.push_str("}\n");

    let mut conf = String::new();
    conf.push_str("# auto-generated by vPanel\n");
    if upgrade {
        conf.push_str("server {\n");
        conf.push_str("    listen 80;\n");
        conf.push_str(&format!("    server_name {};\n", server_name));
        conf.push_str("    return 301 https://$host$request_uri;\n");
        conf.push_str("}\n");
    } else {
        // 保留原 80 块
        conf.push_str(&content);
    }
    conf.push_str(&ssl_block);

    if std::fs::write(&file, conf.as_bytes()).is_err() {
        return (false, format!("写配置文件失败：{}", file));
    }
    let (ok, msg) = nginx_test();
    if !ok {
        return (false, format!("nginx 配置校验失败（未应用）：\n{}", msg));
    }
    let lnk = format!("{}/{}.conf", enabled_dir(), name);
    if !std::path::Path::new(&lnk).exists() {
        let _ = std::process::Command::new("ln").args(["-s", &format!("{}.conf", name), &lnk]).status();
    }
    let (ro, rm) = nginx_reload();
    if ro {
        (true, format!("站点 {} 已启用 HTTPS", name))
    } else {
        (false, format!("配置已写入但 reload 失败：{}", rm))
    }
}

/// 启用 / 停用站点。
pub fn nginx_toggle(name: &str, enable: bool) -> (bool, String) {
    let avail_file = format!("{}/{}.conf", avail_dir(), name);
    if !std::path::Path::new(&avail_file).exists() {
        return (false, format!("在 {} 下未找到站点 {}", avail_dir(), name));
    }
    if !valid_name(name) {
        return (false, "非法的站点名".into());
    }
    let lnk = format!("{}/{}.conf", enabled_dir(), name);
    if enable {
        if std::path::Path::new(&lnk).exists() {
            return (true, format!("站点 {} 已处于启用状态", name));
        }
        let _ = std::process::Command::new("ln").args(["-s", &format!("{}.conf", name), &lnk]).status();
    } else {
        let _ = std::fs::remove_file(&lnk);
    }
    let (ok, msg) = nginx_reload();
    if ok {
        (true, format!("站点 {} 已{}", name, if enable { "启用" } else { "停用" }))
    } else {
        (false, format!("reload 失败：{}", msg))
    }
}

/// 删除站点（停用 + 删配置文件）。
pub fn nginx_delete(name: &str) -> (bool, String) {
    if !valid_name(name) {
        return (false, "非法的站点名".into());
    }
    let avail_file = format!("{}/{}.conf", avail_dir(), name);
    let lnk = format!("{}/{}.conf", enabled_dir(), name);
    if !std::path::Path::new(&avail_file).exists() {
        return (false, format!("站点 {} 不存在", name));
    }
    let _ = std::fs::remove_file(&lnk);
    let _ = std::fs::remove_file(&avail_file);
    let (ok, msg) = nginx_reload();
    if ok {
        (true, format!("站点 {} 已删除", name))
    } else {
        (false, format!("已删除配置文件，但 reload 失败：{}", msg))
    }
}

/// 重新加载 nginx（也用于单独触发）。
pub fn nginx_reload_endpoint() -> (bool, String) {
    nginx_reload()
}

// ---------------------------------------------------------------------------
// 开机自启服务（systemctl enable / disable）
// ---------------------------------------------------------------------------

/// 设置服务开机自启。
pub fn autostart_action(name: &str, enable: bool) -> (bool, String) {
    let action = if enable { "enable" } else { "disable" };
    let out = std::process::Command::new("systemctl")
        .args([action, name])
        .output();
    match out {
        Ok(o) => {
            let msg = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if o.status.success() {
                (true, format!("服务 {} 已{}开机自启", name, if enable { "启用" } else { "关闭" }))
            } else {
                (false, if msg.is_empty() { "操作失败".into() } else { msg })
            }
        }
        Err(e) => (false, e.to_string()),
    }
}

/// 开机自启服务列表 -> JSON（systemctl list-unit-files 中 enabled 的）。
pub fn autostart_json() -> String {
    let out = json::run_out("systemctl", &["list-unit-files", "--type=service", "--no-legend", "--no-pager"]);
    let mut items = Vec::new();
    if let Some(s) = out {
        for line in s.lines() {
            let mut it = line.split_whitespace();
            let name = it.next().unwrap_or("");
            let state = it.next().unwrap_or("");
            // enabled / enabled-runtime / static 等
            if name.ends_with(".service") && state.starts_with("enabled") {
                items.push(format!(
                    "{{\"name\":\"{}\",\"state\":\"{}\"}}",
                    json::jesc(name),
                    json::jesc(state)
                ));
            }
        }
    }
    items.sort();
    format!("{{\"ok\":true,\"list\":[{}]}}", items.join(","))
}