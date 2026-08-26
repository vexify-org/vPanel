//! 运行环境管理：检测常用运行时（PHP/Redis/Node/Python/MySQL/Nginx/Docker/Go）
//! 的安装状态与版本，并支持一键安装（apt/官方脚本）与启停服务。

use crate::json;

/// 执行命令并返回 stdout 首行。
fn sh_out(cmd: &str) -> Option<String> {
    std::process::Command::new("sh").arg("-c").arg(cmd).output().ok().and_then(|o| {
        if o.status.success() {
            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
        } else {
            None
        }
    })
}

/// 判断命令是否可用。
fn has(cmd: &str) -> bool {
    sh_out(&format!("command -v {} >/dev/null 2>&1 && echo yes", cmd)).is_some()
}

/// 取命令版本字符串（首行）。
fn version_of(cmd: &str, ver_flag: &str) -> String {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&format!("{} {} 2>/dev/null | head -n1 || true", cmd, ver_flag))
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn service_active(name: &str) -> bool {
    std::process::Command::new("systemctl")
        .args(["is-active", "--quiet", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// PHP 主版本。
fn php_version() -> String {
    for p in ["php", "php8.2", "php8.1", "php8.0", "php7.4", "php7.2"] {
        if has(p) {
            if let Some(v) = version_of(p, "-v").split_whitespace().nth(1) {
                return v.to_string();
            }
        }
    }
    String::new()
}

/// 单个运行时条目。
struct EnvEntry {
    id: &'static str,
    name: &'static str,
    installed: bool,
    version: String,
    running: bool,
    remark: &'static str,
}

/// 运行环境总览 -> JSON。
pub fn status_json() -> String {
    let entries = vec![
        EnvEntry {
            id: "nginx",
            name: "Nginx",
            installed: has("nginx"),
            version: version_of("nginx", "-v"),
            running: service_active("nginx"),
            remark: "Web / 反向代理",
        },
        EnvEntry {
            id: "mysql",
            name: "MySQL/MariaDB",
            installed: has("mysql") || has("mariadb"),
            version: if has("mysql") { version_of("mysql", "--version") } else { version_of("mariadb", "--version") },
            running: service_active("mysql") || service_active("mariadb"),
            remark: "关系型数据库",
        },
        EnvEntry {
            id: "redis",
            name: "Redis",
            installed: has("redis-server"),
            version: version_of("redis-server", "--version"),
            running: service_active("redis-server"),
            remark: "内存键值数据库",
        },
        EnvEntry {
            id: "php",
            name: "PHP",
            installed: has("php"),
            version: php_version(),
            running: service_active("php-fpm") || service_active("php8.2-fpm") || service_active("php7.4-fpm"),
            remark: "脚本语言（FPM 运行）",
        },
        EnvEntry {
            id: "node",
            name: "Node.js",
            installed: has("node"),
            version: version_of("node", "-v"),
            running: true,
            remark: "JavaScript 运行时",
        },
        EnvEntry {
            id: "docker",
            name: "Docker",
            installed: has("docker"),
            version: version_of("docker", "--version"),
            running: service_active("docker"),
            remark: "容器运行时",
        },
        EnvEntry {
            id: "python",
            name: "Python",
            installed: has("python3"),
            version: version_of("python3", "--version"),
            running: true,
            remark: "通用脚本语言",
        },
        EnvEntry {
            id: "go",
            name: "Go",
            installed: has("go"),
            version: version_of("go", "version"),
            running: true,
            remark: "编译型语言",
        },
    ];

    let list: Vec<serde_json::Value> = entries.iter().map(|e| {
        serde_json::json!({
            "id": e.id,
            "name": e.name,
            "installed": e.installed,
            "version": e.version,
            "running": e.running,
            "remark": e.remark,
        })
    }).collect();

    serde_json::json!({"ok": true, "len": list.len(), "list": list}).to_string()
}

/// 一键安装某运行时。
pub fn install(id: &str) -> (bool, String) {
    let script = match id {
        "nginx" => "set -e\napt-get update -qq\napt-get install -y -qq nginx\nsystemctl enable --now nginx",
        "mysql" => "set -e\napt-get update -qq\nDEBIAN_FRONTEND=noninteractive apt-get install -y -qq mysql-server\nsystemctl enable --now mysql",
        "mariadb" => "set -e\napt-get update -qq\nDEBIAN_FRONTEND=noninteractive apt-get install -y -qq mariadb-server\nsystemctl enable --now mariadb",
        "redis" => "set -e\napt-get update -qq\napt-get install -y -qq redis-server\nsystemctl enable --now redis-server",
        "php" => "set -e\napt-get update -qq\nDEBIAN_FRONTEND=noninteractive apt-get install -y -qq php-fpm php-cli php-mysql php-curl php-gd php-intl php-mbstring php-xml php-zip php-redis\nsystemctl enable php-fpm 2>/dev/null || true",
        "node" => "set -e\ncurl -fsSL \"https://deb.nodesource.com/setup_lts.x\" | bash -\napt-get install -y -qq nodejs",
        "docker" => "set -e\ncurl -fsSL \"https://get.docker.com\" | sh\nsystemctl enable --now docker",
        "go" => "set -e\ncurl -fsSL \"https://go.dev/dl/go1.23.2.linux-amd64.tar.gz\" | tar -C /usr/local -xz\necho 'export PATH=$PATH:/usr/local/go/bin' >> /etc/profile",
        _ => return (false, format!("未知运行时: {}", id)),
    };
    exec_script(script)
}

/// 启动/停止/重启系统服务。
pub fn service(id: &str, action: &str) -> (bool, String) {
    let unit = match id {
        "nginx" => "nginx",
        "mysql" | "mariadb" => if has("mariadb") { "mariadb" } else { "mysql" },
        "redis" => "redis-server",
        "php" => "php-fpm",
        "docker" => "docker",
        _ => return (false, format!("该运行时不支持服务管理: {}", id)),
    };
    if !matches!(action, "start" | "stop" | "restart") {
        return (false, "action 应为 start/stop/restart".into());
    }
    match std::process::Command::new("systemctl").args([action, unit]).output() {
        Ok(o) if o.status.success() => (true, format!("{} {} 成功", unit, action)),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

fn exec_script(script: &str) -> (bool, String) {
    match std::process::Command::new("bash").arg("-c").arg(script).output() {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            if !s.is_empty() { s.push('\n'); }
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            let tail: Vec<&str> = s.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect();
            let msg = if tail.join("\n").trim().is_empty() { "执行完成".into() } else { tail.join("\n").trim().to_string() };
            (o.status.success(), msg)
        }
        Err(e) => (false, e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_detection() {
        assert!(has("sh"));
    }

    #[test]
    fn unknown_install_rejected() {
        let (ok, _) = install("not-a-rt");
        assert!(!ok);
    }
}