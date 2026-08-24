//! 证书（SSL）管理：导入已有证书、签发自签证书、Let's Encrypt(acme.sh)，
//! 并把证书一键套到 nginx 站点（443 + 301 跳转）。
//!
//! 证书统一存放在 `<certs.dir>/<name>/fullchain.pem` 与 `privkey.pem`。

use crate::config::Certs;
use crate::json;

/// 合法证书名：字母数字 / 连字符 / 点 / 下划线。
fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 64
        && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
}

fn dir_of(cfg: &Certs, name: &str) -> String {
    format!("{}/{}", cfg.dir.trim_end_matches('/'), name)
}

/// 导入已有证书（PEM 文本）。
pub fn import(cfg: &Certs, name: &str, fullchain: &str, privkey: &str) -> (bool, String) {
    if !valid_name(name) {
        return (false, "证书名只能含字母/数字/连字符/点/下划线".into());
    }
    if fullchain.trim().is_empty() || privkey.trim().is_empty() {
        return (false, "证书与私钥内容不能为空".into());
    }
    let d = dir_of(cfg, name);
    std::fs::create_dir_all(&d);
    let fc = format!("{}/fullchain.pem", d);
    let pk = format!("{}/privkey.pem", d);
    if std::fs::write(&fc, fullchain).is_err() {
        return (false, format!("写入证书失败: {}", fc));
    }
    if std::fs::write(&pk, privkey).is_err() {
        return (false, format!("写入私钥失败: {}", pk));
    }
    (true, format!("已导入证书 {} -> {}", name, fc))
}

/// 生成自签证书（openssl，默认 365 天）。
pub fn self_signed(cfg: &Certs, name: &str, domain: &str, days: u32) -> (bool, String) {
    if !valid_name(name) {
        return (false, "证书名非法".into());
    }
    let d = domain.trim().to_string();
    let cn = if d.is_empty() { name.to_string() } else { d };
    if !valid_domain(&cn) {
        return (false, "域名不合法".into());
    }
    let dir = dir_of(cfg, name);
    std::fs::create_dir_all(&dir);
    let days = if days == 0 { 365 } else { days };
    let fc = format!("{}/fullchain.pem", dir);
    let pk = format!("{}/privkey.pem", dir);
    let dl = format!("{}/self-signed", dir);
    let key = format!("{}/key-auto.pem", dir);
    let out = std::process::Command::new("openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-nodes",
            "-keyout", &key, "-out", &dl,
            "-days", &days.to_string(), "-subj",
            &format!("/CN={}", cn),
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            // 合并证书为 fullchain（自签单文件即可），私钥用自动生成的。
            match std::fs::copy(&key, &pk).and_then(|_| std::fs::copy(&dl, &fc)) {
                Ok(_) => {
                    let _ = std::fs::remove_file(&key);
                    let _ = std::fs::remove_file(&dl);
                    (true, format!("已生成自签证书 {}（{cn}）", name))
                }
                Err(e) => (false, format!("落盘失败: {}", e)),
            }
        }
        _ => (false, "openssl 生成失败".into()),
    }
}

/// 用 acme.sh 签发 Let's Encrypt 证书（需已安装 acme.sh）。
pub fn le_issue(cfg: &Certs, name: &str, domain: &str, webroot: &str) -> (bool, String) {
    if !valid_name(name) {
        return (false, "证书名非法".into());
    }
    if !valid_domain(domain.trim()) {
        return (false, "域名不合法".into());
    }
    // 校验 acme.sh 是否在。
    let has_acme = std::process::Command::new("sh")
        .arg("-c")
        .arg("command -v acme.sh >/dev/null 2>&1")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !has_acme {
        return (false, "未安装 acme.sh，无法签发。请先安装（curl https://get.acme.sh | sh）".into());
    }
    let dir = dir_of(cfg, name);
    std::fs::create_dir_all(&dir);
    let wd = "--webroot";
    let out = std::process::Command::new("acme.sh")
        .args([
            "--issue", "-d", domain.trim(), wd,
            if webroot.trim().is_empty() { "/var/www" } else { webroot.trim() },
        ])
        .output();
    if !matches!(&out, Ok(o) if o.status.success()) {
        let msg = match &out {
            Ok(o) => String::from_utf8_lossy(&o.stderr).trim().to_string(),
            Err(e) => e.to_string(),
        };
        return (false, if msg.is_empty() { "acme.sh 签发失败".to_string() } else { msg });
    }
    // 拷贝到面板目录
    let rc = format!("{}/fullchain.pem", dir);
    let rk = format!("{}/privkey.pem", dir);
    let cp = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "mkdir -p {} && acme.sh --install-cert -d {} --fullchain-file {} --key-file {} >/dev/null 2>&1",
            shq(&dir), shq(domain.trim()), shq(&rc), shq(&rk)
        ))
        .status();
    if cp.map(|s| s.success()).unwrap_or(false) {
        (true, format!("已签发 Let's Encrypt 证书 {}（{domain}）", name))
    } else {
        (false, "签发成功但拷贝证书失败".into())
    }
}

/// 列出所有证书 + 已套用的站点。
pub fn list_json(cfg: &Certs) -> String {
    let mut items = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&cfg.dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !e.path().is_dir() {
                continue;
            }
            let fc = format!("{}/{}/fullchain.pem", cfg.dir.trim_end_matches('/'), name);
            let ok = std::path::Path::new(&fc).is_file();
            let mtime = e.metadata()
                .and_then(|m| m.modified()).ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let (domain, not_after) = read_cert_info(&fc);
            items.push(format!(
                "{{\"name\":\"{}\",\"ok\":{},\"domain\":\"{}\",\"not_after\":\"{}\",\"mtime\":{}}}",
                json::jesc(&name),
                ok,
                json::jesc(&domain),
                json::jesc(&not_after),
                mtime
            ));
        }
    }
    items.sort();
    format!("{{\"ok\":true,\"dir\":\"{}\",\"list\":[{}]}}", json::jesc(&cfg.dir), items.join(","))
}

/// 读取证书 subject CN 与有效期（尽力而为，供前端展示）。
fn read_cert_info(fullchain: &str) -> (String, String) {
    if !std::path::Path::new(fullchain).is_file() {
        return (String::new(), String::new());
    }
    let out = std::process::Command::new("openssl")
        .args(["x509", "-in", fullchain, "-noout", "-subject", "-enddate"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut cn = String::new();
    let mut end = String::new();
    for line in out.lines() {
        if line.starts_with("subject=") {
            if let Some(i) = line.find("CN=") {
                cn = line[i + 3..].to_string();
            }
        } else if let Some(rest) = line.strip_prefix("notAfter=") {
            end = rest.to_string();
        }
    }
    (cn.trim().to_string(), end)
}

/// 把某个证书套到 nginx 站点上。
pub fn apply(cfg: &Certs, site: &str, cert_name: &str, upgrade: bool) -> (bool, String) {
    if !valid_name(site) {
        return (false, "站点名非法".into());
    }
    if !valid_name(cert_name) {
        return (false, "证书名非法".into());
    }
    let d = dir_of(cfg, cert_name);
    let fc = format!("{}/fullchain.pem", d);
    let pk = format!("{}/privkey.pem", d);
    if !std::path::Path::new(&fc).is_file() || !std::path::Path::new(&pk).is_file() {
        return (false, format!("证书 {} 不存在或未生成", cert_name));
    }
    crate::nginx::nginx_ssl(site, &fc, &pk, upgrade)
}

fn valid_domain(d: &str) -> bool {
    !d.is_empty()
        && d.len() <= 253
        && d.split('.').all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && d.contains('.')
}

fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "\\'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_valid() {
        assert!(valid_domain("example.com"));
        assert!(valid_domain("a.b.example.com.cn"));
        assert!(!valid_domain("example"));
        assert!(!valid_domain(""));
        assert!(!valid_domain("exa mple.com"));
    }

    #[test]
    fn names_valid() {
        assert!(valid_name("my-cert-1"));
        assert!(!valid_name("a;b"));
        assert!(!valid_name(""));
    }
}