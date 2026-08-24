//! 运行环境（对标宝塔「软件商店 / 运行环境」）：
//! 检测常用运行时（PHP/Redis/Node/Python/MySQL/Nginx/Docker/Go）的安装状态与版本，
//! 并支持一键安装（apt/官方脚本）与启停服务。

use crate::json;

fn sh_out(cmd: &str) -> Option<String> {
    let out = std::process::Command::new("sh").arg("-c").arg(cmd).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

/// 判断命令是否可用。
pub fn has(cmd: &str) -> bool {
    sh_out(&format!("command -v {} >/dev/null 2>&1 && echo yes", cmd)).is_some()
}

/// 取命令版本字符串（首行，截断）。命令不存在/失败返回空串。
pub fn version_of(cmd: &str, ver_flag: &str) -> String {
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(&format!("{} {} 2>/dev/null | head -n1 || true", cmd, ver_flag))
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::new(),
    }
}

fn service_active(name: &str) -> bool {
    let out = std::process::Command::new("systemctl")
        .args(["is-active", "--quiet", name])
        .status();
    matches!(out, Ok(s) if s.success())
}

/// PHP 主版本（如 8.2）。遍历常见 php-fpm 变体。
pub fn php_version() -> String {
    for p in ["php", "php8.2", "php8.1", "php8.0", "php7.4", "php7.2"] {
        if has(p) {
            let v = version_of(p, "-v");
            if let Some(f) = v.split_whitespace().nth(1) {
                return f.to_string();
            }
        }
    }
    String::new()
}

/// 运行环境总览 -> JSON。
pub fn status_json() -> String {
    // 每项：id, name, installed(bool), version, running(bool), remark
    let entries: Vec<(String, String, bool, String, bool, String)> = vec![
        (
            "nginx".into(),
            "Nginx".into(),
            has("nginx"),
            version_of("nginx", "-v"),
            service_active("nginx"),
            "Web / 反向代理".into(),
        ),
        (
            "mysql".into(),
            "MySQL/MariaDB".into(),
            has("mysql") || has("mariadb"),
            if has("mysql") { version_of("mysql", "--version") } else { version_of("mariadb", "--version") },
            service_active("mysql") || service_active("mariadb"),
            "关系型数据库".into(),
        ),
        (
            "redis".into(),
            "Redis".into(),
            has("redis-server"),
            version_of("redis-server", "--version"),
            service_active("redis-server"),
            "内存键值数据库".into(),
        ),
        (
            "php".into(),
            "PHP".into(),
            has("php"),
            php_version(),
            service_active("php-fpm") || service_active("php8.2-fpm") || service_active("php7.4-fpm"),
            "脚本语言（FPM 运行）。宝塔多版本在此面板下以系统包落地".into(),
        ),
        (
            "node".into(),
            "Node.js".into(),
            has("node"),
            version_of("node", "-v"),
            true,
            "JavaScript 运行时".into(),
        ),
        (
            "docker".into(),
            "Docker".into(),
            has("docker"),
            version_of("docker", "--version"),
            service_active("docker"),
            "容器运行时".into(),
        ),
        (
            "python".into(),
            "Python".into(),
            has("python3"),
            version_of("python3", "--version"),
            true,
            "通用脚本语言".into(),
        ),
        (
            "go".into(),
            "Go".into(),
            has("go"),
            version_of("go", "version"),
            true,
            "编译型语言".into(),
        ),
    ];

    let mut out = String::from("{\"ok\":true,\"len\":");
    out.push_str(&entries.len().to_string());
    out.push_str(",\"list\":[");
    for (i, (id, name, installed, ver, running, remark)) in entries.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"id\":\"{}\",\"name\":\"{}\",\"installed\":{},\"version\":\"{}\",\"running\":{},\"remark\":\"{}\"}}",
            json::jesc(id),
            json::jesc(name),
            installed,
            json::jesc(ver),
            running,
            json::jesc(remark),
        ));
    }
    out.push_str("]}");
    out
}

/// 一键安装某运行时。支持：nginx/mysql/redis/php/node/docker/go。
pub fn install(id: &str) -> (bool, String) {
    let script = match id {
        "nginx" => "set -e\napt-get update -qq\napt-get install -y -qq nginx\nsystemctl enable --now nginx\n",
        "mysql" => "set -e\napt-get update -qq\nDEBIAN_FRONTEND=noninteractive apt-get install -y -qq mysql-server\nsystemctl enable --now mysql\n",
        "mariadb" => "set -e\napt-get update -qq\nDEBIAN_FRONTEND=noninteractive apt-get install -y -qq mariadb-server\nsystemctl enable --now mariadb\n",
        "redis" => "set -e\napt-get update -qq\napt-get install -y -qq redis-server\nsystemctl enable --now redis-server\n",
        "php" => "set -e\napt-get update -qq\nDEBIAN_FRONTEND=noninteractive apt-get install -y -qq php-fpm php-cli php-mysql php-curl php-gd php-intl php-mbstring php-xml php-zip php-redis\nsystemctl enable php-fpm 2>/dev/null || true\n",
        "node" => "set -e\ncurl -fsSL \"https://deb.nodesource.com/setup_lts.x\" | bash -\napt-get install -y -qq nodejs\n",
        "docker" => "set -e\ncurl -fsSL \"https://get.docker.com\" | sh\nsystemctl enable --now docker\n",
        "go" => "set -e\ncurl -fsSL \"https://go.dev/dl/go1.23.2.linux-amd64.tar.gz\" | tar -C /usr/local -xz\necho 'export PATH=$PATH:/usr/local/go/bin' >> /etc/profile\n",
        _ => return (false, format!("未知运行时: {}", id)),
    };
    exec_script(script)
}

/// 启动/停止/重启系统服务。返回 (是否成功, 说明)。
pub fn service(id: &str, action: &str) -> (bool, String) {
    let unit = match id {
        "nginx" => "nginx".to_string(),
        "mysql" | "mariadb" => if has("mariadb") { "mariadb".into() } else { "mysql".into() },
        "redis" => "redis-server".to_string(),
        "php" => "php-fpm".to_string(),
        "docker" => "docker".to_string(),
        _ => return (false, format!("该运行时不支持服务管理: {}", id)),
    };
    if !matches!(action, "start" | "stop" | "restart") {
        return (false, "action 应为 start/stop/restart".into());
    }
    let out = std::process::Command::new("systemctl")
        .args([action, &unit])
        .output();
    match out {
        Ok(o) if o.status.success() => (true, format!("{} {} 成功", unit, action)),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

fn exec_script(script: &str) -> (bool, String) {
    let out = std::process::Command::new("bash").arg("-c").arg(script).output();
    match out {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            let tail = s.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
            (o.status.success(), if tail.trim().is_empty() { "执行完成".into() } else { tail.trim().to_string() })
        }
        Err(e) => (false, e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_detection() {
        // 只断言不 panic；具体结果依赖环境。
        assert!(has("sh"));
    }

    #[test]
    fn unknown_install_rejected() {
        let (ok, _) = install("not-a-rt");
        assert!(!ok);
    }
}