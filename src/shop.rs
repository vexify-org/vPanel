//! 软件商店：内置常用软件清单 + 一键下载/安装。
//!
//! 设计要点：
//! - 内置精简软件清单，不常驻额外内存。
//! - 下载统一走加速前缀（默认 `https://g.z321.cc.cd/`，配置可改）。
//! - 安装脚本按需 `sh -c` 一次性执行，与面板常驻内存解耦。

use crate::config::Config;

/// 一个可选装的软件项。
struct App {
    id: &'static str,
    name: &'static str,
    desc: &'static str,
    /// 安装脚本模板。`{url}`（可选）替换为「加速前缀+直链」，从而统一加速下载。
    script: &'static str,
}

const ACCEL_PH: &str = "{accel}";

/// 内置软件清单（精简、常见）。
const APPS: &[App] = &[
    App {
        id: "nginx",
        name: "Nginx",
        desc: "高性能 Web / 反向代理服务器",
        script: r#"set -e
apt-get update -qq
apt-get install -y -qq nginx"#,
    },
    App {
        id: "docker",
        name: "Docker",
        desc: "容器运行时（官方脚本，走加速）",
        script: r#"set -e
curl -fsSL "{accel}https://get.docker.com" | sh"#,
    },
    App {
        id: "redis",
        name: "Redis",
        desc: "内存键值数据库",
        script: r#"set -e
apt-get update -qq && apt-get install -y -qq redis-server"#,
    },
    App {
        id: "mysql",
        name: "MySQL",
        desc: "关系型数据库",
        script: r#"set -e
apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y -qq mysql-server"#,
    },
    App {
        id: "go",
        name: "Go",
        desc: "Go 语言工具链（官方 tarball，走加速）",
        script: r#"set -e
curl -fsSL "{accel}https://go.dev/dl/go1.23.2.linux-amd64.tar.gz" | tar -C /usr/local -xz
echo 'export PATH=$PATH:/usr/local/go/bin' >> /etc/profile"#,
    },
    App {
        id: "node",
        name: "Node.js",
        desc: "Node.js 运行时（NodeSource 脚本，走加速）",
        script: r#"set -e
curl -fsSL "{accel}https://deb.nodesource.com/setup_lts.x" | bash -
apt-get install -y -qq nodejs"#,
    },
    App {
        id: "gitlab",
        name: "GitLab CE",
        desc: "自托管 Git 仓库（官方脚本）",
        script: r#"set -e
curl -fsSL "{accel}https://packages.gitlab.com/install/repositories/gitlab/gitlab-ee/script.deb.sh" | bash"#,
    },
    App {
        id: "fail2ban",
        name: "Fail2ban",
        desc: "暴力破解防护",
        script: r#"set -e
apt-get update -qq && apt-get install -y -qq fail2ban
systemctl enable --now fail2ban"#,
    },
];

/// 取加速前缀（去掉末尾 `/`），保证以 `/` 结尾的可拼接形式。
fn accel_of(cfg: &Config) -> String {
    let a = cfg.download.accel.trim().trim_end_matches('/');
    if a.is_empty() {
        DEFAULT_ACCEL.to_string()
    } else {
        format!("{}/", a)
    }
}

/// 商店列表 -> JSON。
pub fn shop_json(cfg: &Config) -> String {
    let accel = accel_of(cfg);
    let mut out = format!(
        "{{\"accel\":\"{}\",\"len\":{}",
        crate::json::jesc(accel.trim_end_matches('/')),
        APPS.len()
    );
    out.push_str(",\"list\":[");
    for (i, a) in APPS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"id\":\"{}\",\"name\":\"{}\",\"desc\":\"{}\"}}",
            crate::json::jesc(a.id),
            crate::json::jesc(a.name),
            crate::json::jesc(a.desc),
        ));
    }
    out.push_str("]}");
    out
}

/// 一键安装某软件。返回 (成功, 输出末尾文本)。
pub fn install(app_id: &str, cfg: &Config) -> (bool, String) {
    let app = match APPS.iter().find(|a| a.id == app_id) {
        Some(a) => a,
        None => return (false, format!("未知软件: {}", app_id)),
    };
    let accel = accel_of(cfg);
    let script = app.script.replace(ACCEL_PH, &accel);
    // 直接 sh -c 执行多行脚本。
    let out = std::process::Command::new("bash")
        .arg("-c")
        .arg(&script)
        .output();
    match out {
        Ok(o) => {
            let tail = chunk_tail(&o.stdout, &o.stderr, 600);
            (o.status.success(), tail)
        }
        Err(e) => (false, e.to_string()),
    }
}

/// 取 stdout+stderr 的末尾文本（限制长度）。
fn chunk_tail(stdout: &[u8], stderr: &[u8], max: usize) -> String {
    let mut s = String::from_utf8_lossy(stdout).into_owned();
    if !s.is_empty() {
        s.push('\n');
    }
    s.push_str(&String::from_utf8_lossy(stderr));
    if s.chars().count() > max {
        s = s.chars().skip(s.chars().count() - max).collect();
        s = "[…截断] ".to_string() + &s;
    }
    s.trim().to_string()
}

/// 检测加速源是否可达 -> JSON。用一个 GitHub raw 小文件探活。
pub fn accel_check(cfg: &Config) -> String {
    let accel = accel_of(cfg);
    let test_url = format!(
        "{}https://raw.githubusercontent.com/github/gitignore/main/README.md",
        accel
    );
    let ok = std::process::Command::new("curl")
        .args(["-fsSL", "-m", "10", "-o", "/dev/null", &test_url])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    format!(
        "{{\"ok\":{},\"accel\":\"{}\"}}",
        ok,
        crate::json::jesc(accel.trim_end_matches('/'))
    )
}

pub const DEFAULT_ACCEL: &str = "https://g.z321.cc.cd/";