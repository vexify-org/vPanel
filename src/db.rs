//! 数据库管理（MySQL/MariaDB）：安装检测、建库建用户、授权、备份/恢复。
//!
//! 通过 mysql / mysqldump 命令行驱动，不在面板进程里引入查询引擎，保持低常驻内存。
//! 密码经 `MYSQL_PWD` 环境变量传递，避免出现在进程列表里。
//! 所有来自用户输入的库名 / 用户名先经 `sani` 清洗，防 SQL 注入。

use crate::config::Database;
use crate::json;

/// 数据库是否已安装（能在 PATH 找到 mysql 客户端）。
pub fn installed(cfg: &Database) -> bool {
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {} >/dev/null 2>&1", shq(&cfg.bin)))
        .status();
    matches!(out, Ok(s) if s.success())
}

/// 数据库服务是否在运行。
pub fn server_running(_cfg: &Database) -> bool {
    let out = std::process::Command::new("pgrep")
        .arg("-x")
        .arg("mysqld")
        .status();
    matches!(out, Ok(s) if s.success())
}

/// 清洗标识符：只保留小写字母/数字/下划线，杜绝注入。
pub fn sani(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// 执行单条 SQL，返回 (是否成功, 输出或错误)。空输出视为成功（建库/授权类）。
fn run_sql(cfg: &Database, sql: &str) -> (bool, String) {
    let sql = sql.trim().trim_end_matches(';').to_string() + ";";
    let mut cmd = std::process::Command::new(&cfg.bin);
    cmd.args(["--batch", "--skip-column-names"]);
    if !cfg.password.is_empty() {
        cmd.env("MYSQL_PWD", &cfg.password);
    }
    cmd.arg(format!("-u{}", cfg.user)).arg("-e").arg(sql);
    match cmd.output() {
        Ok(o) if o.status.success() => {
            (true, String::from_utf8_lossy(&o.stdout).trim().to_string())
        }
        Ok(o) => (
            false,
            String::from_utf8_lossy(&o.stderr)
                .lines()
                .next()
                .unwrap_or("执行失败")
                .trim()
                .to_string(),
        ),
        Err(e) => (false, e.to_string()),
    }
}

/// 列出所有数据库（排除系统库）。
pub fn databases(cfg: &Database) -> (bool, String) {
    let (ok, out) = run_sql(
        cfg,
        "SHOW DATABASES WHERE `Database` NOT IN ('information_schema','mysql','performance_schema','sys')",
    );
    let list: Vec<String> = out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let json = format!(
        "[{}]",
        list.iter().map(|d| format!("\"{}\"", json::jesc(d))).collect::<Vec<_>>().join(",")
    );
    (ok, json)
}

/// 列出所有账号 host/user。
pub fn users(cfg: &Database) -> (bool, String) {
    let (ok, out) = run_sql(
        cfg,
        "SELECT User,Host FROM mysql.user WHERE User NOT IN ('mysql.sys','root') ORDER BY User",
    );
    let arr: Vec<String> = out
        .lines()
        .filter_map(|l| {
            let mut it = l.split('\t');
            let u = it.next()?.to_string();
            let h = it.next().unwrap_or("localhost").to_string();
            Some(format!("{{\"user\":\"{}\",\"host\":\"{}\"}}", json::jesc(&u), json::jesc(&h)))
        })
        .collect();
    (ok, format!("[{}]", arr.join(",")))
}

/// 建库，可选字符集。
pub fn create_db(cfg: &Database, name: &str, charset: &str) -> (bool, String) {
    let n = sani(name);
    if n.is_empty() {
        return (false, "非法数据库名".to_string());
    }
    let cs = if charset.trim().is_empty() { "utf8mb4".to_string() } else { charset.trim().to_string() };
    run_sql(cfg, &format!("CREATE DATABASE IF NOT EXISTS `{n}` DEFAULT CHARACTER SET {cs}"))
}

/// 删除数据库。
pub fn drop_db(cfg: &Database, name: &str) -> (bool, String) {
    let n = sani(name);
    if n.is_empty() {
        return (false, "非法数据库名".to_string());
    }
    run_sql(cfg, &format!("DROP DATABASE IF EXISTS `{n}`"))
}

/// 建账号（仅建号 + 授权指定库，与宝塔的「建站同时建库建用户」同一模型）。
pub fn create_user(cfg: &Database, user: &str, pass: &str, host: &str) -> (bool, String) {
    let u = sani(user);
    let h = if host.trim().is_empty() { "localhost".to_string() } else { sani(host) };
    if u.is_empty() {
        return (false, "非法用户名".to_string());
    }
    if h.is_empty() {
        return (false, "非法 host".to_string());
    }
    // 一次性密码：允许任意，但清洗非法字符，避免注入到引号串。
    let p = pass.trim().chars().filter(|c| *c != '\'' && *c != ';').collect::<String>();
    let (ok, msg) = run_sql(
        cfg,
        &format!("CREATE USER IF NOT EXISTS '{u}'@'{h}' IDENTIFIED BY '{p}'"),
    );
    if !ok {
        return (ok, msg);
    }
    run_sql(cfg, &format!("FLUSH PRIVILEGES"))
}

/// 授权：将某库的所有权限授予账号。
pub fn grant(cfg: &Database, db: &str, user: &str, host: &str) -> (bool, String) {
    let d = sani(db);
    let u = sani(user);
    let h = if host.trim().is_empty() { "localhost".to_string() } else { sani(host) };
    if d.is_empty() || u.is_empty() || h.is_empty() {
        return (false, "参数不合法".to_string());
    }
    let r = run_sql(cfg, &format!("GRANT ALL PRIVILEGES ON `{d}`.* TO '{u}'@'{h}'"));
    let _ = run_sql(cfg, "FLUSH PRIVILEGES");
    r
}

/// 删除账号。
pub fn drop_user(cfg: &Database, user: &str, host: &str) -> (bool, String) {
    let u = sani(user);
    let h = if host.trim().is_empty() { "localhost".to_string() } else { sani(host) };
    if u.is_empty() || h.is_empty() {
        return (false, "参数不合法".to_string());
    }
    run_sql(cfg, &format!("DROP USER IF EXISTS '{u}'@'{h}'"))
}

/// 备份指定数据库到 `backup_dir/<db>_<时间戳>.sql.gz`。
pub fn backup(cfg: &Database, db: &str, dest_dir: &str) -> (bool, String) {
    if !installed(cfg) {
        return (false, "未检测到 MySQL 客户端".to_string());
    }
    let d = sani(db);
    if d.is_empty() {
        return (false, "非法数据库名".to_string());
    }
    std::fs::create_dir_all(dest_dir);
    let ts = now_stamp();
    let gz = format!("{}/{}_{}.sql.gz", dest_dir.trim_end_matches('/'), d, ts);
    // mysqldump -> gzip
    let mut pipe = std::process::Command::new("bash")
        .arg("-c")
        .arg(format!(
            "MYSQL_PWD={} {} -u{} {} 2>/dev/null | gzip -c > {}",
            shq(&cfg.password),
            shq(&cfg.dump),
            cfg.user,
            shq(&d),
            shq(&gz),
        ))
        .spawn();
    match pipe {
        Ok(_) => (true, format!("已备份到 {}", gz)),
        Err(e) => (false, format!("备份失败: {}", e)),
    }
}

/// 恢复：把 SQL（含 gzip 压缩）导入指定库。
pub fn restore(cfg: &Database, db: &str, file: &str) -> (bool, String) {
    let d = sani(db);
    if d.is_empty() {
        return (false, "非法数据库名".to_string());
    }
    // bash 管道：gz 先解压再喂给 mysql。
    let load = if file.ends_with(".gz") {
        format!("gzip -dc {} | {} -u{} {}", shq(file), shq(&cfg.bin), cfg.user, shq(&d))
    } else {
        format!("{} -u{} {} < {}", shq(&cfg.bin), cfg.user, shq(&d), shq(file))
    };
    let script = format!("set -o pipefail; MYSQL_PWD={} {}", shq(&cfg.password), load);
    let out = std::process::Command::new("bash").arg("-c").arg(&script).output();
    match out {
        Ok(o) if o.status.success() => (true, format!("已恢复 {} 到 {}", d, file)),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// 当前时间戳（YYYYmmdd_HHMMSS，取本地时间）。
fn now_stamp() -> String {
    if let Ok(o) = std::process::Command::new("date").arg("+%Y%m%d_%H%M%S").output() {
        if let Ok(s) = String::from_utf8(o.stdout) {
            let t = s.trim().to_string();
            if !t.is_empty() {
                return t;
            }
        }
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

/// 单引号转义，用于拼接进 shell。
fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "\\'"))
}

/// 重置 MySQL 的 root 用户密码（通过 ALTER USER，用当前密码经 MYSQL_PWD 传递）。
/// 注意：若旧密码未知/错误，本调用会失败；如需彻底重置请走面板外维护（--skip-grant-tables）。
pub fn reset_root_password(cfg: &Database, new_password: &str) -> (bool, String) {
    let npw = new_password.trim();
    if npw.len() < 4 {
        return (false, "新密码至少 4 位".to_string());
    }
    let script = format!(
        "MYSQL_PWD={} {} -u{} -e \"ALTER USER 'root'@'localhost' IDENTIFIED BY '{}'\"",
        shq(&cfg.password),
        shq(&cfg.bin),
        cfg.user,
        npw.replace('\'', "\\'")
    );
    let out = std::process::Command::new("bash").arg("-c").arg(&script).output();
    match out {
        Ok(o) if o.status.success() => (true, "root 密码已重置".to_string()),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_names() {
        assert_eq!(sani(" My_DB-1 "), "my_db1");
        assert_eq!(sani("a;DROP TABLE x"), "adroptablex");
        assert_eq!(sani(""), "");
        assert_eq!(sani("__"), "__");
    }

    #[test]
    fn shquote_escapes() {
        assert_eq!(shq("abc"), "'abc'");
        assert_eq!(shq("a'b"), "'a\\'b'");
    }

    #[test]
    fn now_stamp_nonempty() {
        assert!(!now_stamp().is_empty());
    }
}