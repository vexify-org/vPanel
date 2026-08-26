//! 安全加固：WAF（nginx 限流/恶意 UA 拦截）、IP 封禁（ufw）、
//! 暴力破解扫描封禁、SSH 加固（禁止 root 密码登录/关闭密码认证）。

use crate::json;

const BAN_FILE: &str = ".vpanel-ban.json";
const WAF_FILE: &str = "conf.d/vpanel-waf.conf";
const HARDEN_FILE: &str = "/etc/ssh/sshd_config.d/vpanel.conf";
const HARDEN_FILE_ALT: &str = "/etc/ssh/sshd_config.d/99-vpanel.conf";

fn ban_path() -> String {
    format!("{}/{}", crate::config::Config::panel_dir(), BAN_FILE)
}

/// IP 校验（IPv4/IPv6）。
pub fn is_ip(ip: &str) -> bool {
    let s = ip.trim();
    if s.contains(':') { return !s.is_empty() && !s.contains(' ') && !s.contains('/'); }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 { return false; }
    parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) && p.parse::<u32>().map_or(false, |n| n <= 255))
}

/// 封禁 IP：写 ufw deny + 落盘。
pub fn ban_ip(ip: &str) -> (bool, String) {
    if !is_ip(ip) { return (false, "非法的 IP".into()); }
    match std::process::Command::new("ufw").args(["deny", ip.trim()]).output() {
        Ok(o) if o.status.success() => {
            add_to_ban_file(ip.trim());
            (true, format!("已封禁 {}", ip.trim()))
        }
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// 解封 IP。
pub fn unban_ip(ip: &str) -> (bool, String) {
    if !is_ip(ip) { return (false, "非法的 IP".into()); }
    match std::process::Command::new("ufw").args(["delete", "deny", ip.trim()]).output() {
        Ok(o) if o.status.success() => {
            remove_from_ban_file(ip.trim());
            (true, format!("已解封 {}", ip.trim()))
        }
        _ => (false, "解封失败".into()),
    }
}

fn add_to_ban_file(ip: &str) {
    let mut list = load_bans();
    if !list.iter().any(|x| x == ip) { list.push(ip.to_string()); save_bans(&list); }
}

fn remove_from_ban_file(ip: &str) {
    let mut list = load_bans();
    list.retain(|x| x != ip);
    save_bans(&list);
}

fn load_bans() -> Vec<String> {
    std::fs::read_to_string(ban_path()).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_bans(list: &[String]) {
    if let Ok(s) = serde_json::to_string(list) { let _ = std::fs::write(ban_path(), s); }
}

/// 已封禁 IP 列表 -> JSON。
pub fn bans_json() -> String {
    let list = load_bans();
    let arr: Vec<serde_json::Value> = list.iter().map(|x| serde_json::json!(x)).collect();
    serde_json::json!({"ok": true, "list": arr}).to_string()
}

/// 扫描认证失败日志，超阈值的源 IP 自动封禁。
pub fn brute_scan(threshold: u32) -> String {
    let threshold = if threshold == 0 { 5 } else { threshold };
    let log = read_auth_log();
    let mut count: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for line in log.lines() {
        if let Some(ip) = failed_ip_of(line) { *count.entry(ip).or_insert(0) += 1; }
    }
    let mut ipv: Vec<(&String, &u32)> = count.iter().collect();
    ipv.sort_by(|a, b| b.1.cmp(a.1));
    let mut banned = Vec::new();
    for (ip, n) in &ipv {
        if **n >= threshold {
            let (ok, _) = ban_ip(ip);
            if ok { banned.push(serde_json::json!({"ip": ip, "times": n})); }
        }
    }
    serde_json::json!({"ok": true, "threshold": threshold, "scanned": count.len(), "banned": banned}).to_string()
}

/// 从一行 auth 日志提取失败密码的源 IP。
fn failed_ip_of(line: &str) -> Option<String> {
    let has_failed = line.contains("Failed password") || line.contains("Failed ")
        || line.contains("authentication failure") || line.contains("Invalid user")
        || line.contains("Unknown user") || line.contains("connection reset");
    if !has_failed && !line.contains("rhost=") { return None; }
    if let Some(i) = line.find(" from ") {
        let word = line[i + " from ".len()..].split_whitespace().next().unwrap_or("");
        if is_ip(word) { return Some(word.to_string()); }
    }
    if let Some(i) = line.find("rhost=") {
        let word = line[i + "rhost=".len()..].split_whitespace().next().unwrap_or("").trim_end_matches(",user=none");
        if is_ip(word) { return Some(word.to_string()); }
    }
    None
}

fn read_auth_log() -> String {
    for p in ["/var/log/auth.log", "/var/log/secure"] {
        if let Ok(s) = std::fs::read_to_string(p) { return s; }
    }
    std::process::Command::new("journalctl")
        .args(["-u", "sshd", "--no-pager", "-n", "5000"])
        .output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// WAF（nginx 全局片段）
// ---------------------------------------------------------------------------

/// 生成 nginx WAF 片段。
pub fn waf_conf(rps: u32, burst: u32, banned_ips: &[String]) -> String {
    let rps = if rps == 0 { 20 } else { rps };
    let _burst = if burst == 0 { 40 } else { burst };
    let mut s = String::from("# vPanel WAF (auto-generated)\n");
    s.push_str(&format!("limit_req_zone $binary_remote_addr zone=vpanel_req:16m rate={}r/s;\n", rps));
    s.push_str("limit_conn_zone $binary_remote_addr zone=vpanel_conn:16m;\n");
    s.push_str("map $http_user_agent $vpanel_bad_ua {\n    default 0;\n    ~*sqlmap 1;\n    ~*nikto 1;\n    ~*nmap 1;\n    ~*masscan 1;\n    ~*curl 0;\n}\n");
    if !banned_ips.is_empty() {
        s.push_str("geo $vpanel_banned {\n    default 0;\n");
        for ip in banned_ips { s.push_str(&format!("    {} 1;\n", ip)); }
        s.push_str("}\n");
    }
    s
}

/// 写 WAF 片段到 nginx 并 reload。
pub fn waf_apply(rps: u32, burst: u32) -> (bool, String) {
    let conf = format!("{}/{}", crate::nginx::conf_dir(), WAF_FILE);
    let banned = load_bans();
    let content = waf_conf(rps, burst, &banned);
    if std::fs::write(&conf, content.as_bytes()).is_err() {
        return (false, format!("写 WAF 配置失败: {}", conf));
    }
    let (ok, msg) = crate::nginx::nginx_test();
    if !ok { return (false, format!("WAF 配置校验失败：\n{}", msg)); }
    let (ro, rm) = crate::nginx::nginx_reload();
    if ro {
        (true, "WAF 已启用（限速 + 连接限制 + 恶意 UA 拦截 + IP 黑名单）".into())
    } else {
        (false, format!("WAF 已写入但 reload 失败：{}", rm))
    }
}

/// 关闭 WAF（删片段 + reload）。
pub fn waf_disable() -> (bool, String) {
    let conf = format!("{}/{}", crate::nginx::conf_dir(), WAF_FILE);
    let _ = std::fs::remove_file(&conf);
    let (ro, rm) = crate::nginx::nginx_reload();
    if ro { (true, "WAF 已关闭".into()) } else { (false, format!("reload 失败：{}", rm)) }
}

// ---------------------------------------------------------------------------
// 系统加固（SSH）
// ---------------------------------------------------------------------------

/// 加固开关状态。
pub fn hardening_status() -> String {
    let on = [HARDEN_FILE, HARDEN_FILE_ALT].iter().any(|p| std::path::Path::new(p).is_file());
    serde_json::json!({"ok": true, "on": on}).to_string()
}

/// 开启 SSH 加固。
pub fn harden_ssh(no_root_pass: bool, no_password: bool) -> (bool, String) {
    let mut lines = String::from("# vPanel hardening (auto-generated)\n");
    if no_root_pass { lines.push_str("PermitRootLogin prohibit-password\n"); }
    if no_password {
        lines.push_str("PasswordAuthentication no\n");
        lines.push_str("ChallengeResponseAuthentication no\n");
        lines.push_str("KbdInteractiveAuthentication no\n");
    }
    let dir = std::path::Path::new("/etc/ssh/sshd_config.d");
    let _ = std::fs::create_dir_all(dir);
    let target = if std::path::Path::new(HARDEN_FILE_ALT).exists() { HARDEN_FILE_ALT } else { HARDEN_FILE };
    if std::fs::write(target, lines.as_bytes()).is_err() { return (false, format!("写入 {} 失败", target)); }
    match std::process::Command::new("sshd").args(["-t"]).output() {
        Ok(o) if o.status.success() => {
            let _ = std::process::Command::new("systemctl").args(["reload", "ssh"]).status();
            let _ = std::process::Command::new("systemctl").args(["reload", "sshd"]).status();
            (true, format!("SSH 加固已开启 -> {}", target))
        }
        Ok(o) => {
            let _ = std::fs::remove_file(target);
            (false, format!("sshd -t 校验失败，已回滚：\n{}", String::from_utf8_lossy(&o.stderr).trim()))
        }
        Err(e) => (false, format!("sshd 不存在或不可用：{}", e)),
    }
}

/// 关闭 SSH 加固。
pub fn unharden_ssh() -> (bool, String) {
    let mut removed = false;
    for f in [HARDEN_FILE, HARDEN_FILE_ALT] {
        if std::path::Path::new(f).exists() { let _ = std::fs::remove_file(f); removed = true; }
    }
    if removed {
        let _ = std::process::Command::new("systemctl").args(["reload", "ssh"]).status();
        let _ = std::process::Command::new("systemctl").args(["reload", "sshd"]).status();
        (true, "SSH 加固已关闭".into())
    } else {
        (false, "尚未开启加固，无需关闭".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_validation() {
        assert!(is_ip("1.2.3.4"));
        assert!(is_ip("0.0.0.0"));
        assert!(is_ip("255.255.255.255"));
        assert!(!is_ip("256.1.1.1"));
        assert!(!is_ip("1.2.3"));
        assert!(!is_ip("1.2.3.4.5"));
        assert!(!is_ip("abc"));
        assert!(!is_ip(""));
    }

    #[test]
    fn extract_failed_ip() {
        assert_eq!(failed_ip_of("Sep  1 10:00:00 host sshd[1]: Failed password for root from 1.2.3.4 port 22"), Some("1.2.3.4".to_string()));
        assert_eq!(failed_ip_of("Dec 1 12:00:00 host sshd[2]: Invalid user foo from 2001:db8::1 port 22"), Some("2001:db8::1".to_string()));
        assert_eq!(failed_ip_of("random system line"), None);
    }

    #[test]
    fn waf_generation() {
        let c = waf_conf(10, 30, &["1.2.3.4".to_string()]);
        assert!(c.contains("limit_req_zone"));
        assert!(c.contains("10r/s"));
        assert!(c.contains("map $http_user_agent"));
        assert!(c.contains("1.2.3.4 1;"));
    }
}