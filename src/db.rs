//! 数据库管理（MySQL/MariaDB）：安装检测、建库建用户、授权、备份/恢复。
//!
//! 通过 mysql / mysqldump 命令行驱动，不在面板进程里引入查询引擎，保持低常驻内存。
//! 密码经 `MYSQL_PWD` 环境变量传递，避免出现在进程列表里。
//! 所有来自用户输入的库名/用户名先经 `sani` 清洗，防 SQL 注入。

use crate::config::Database;
use crate::json;

/// 数据库是否已安装（能在 PATH 找到 mysql 客户端）。
pub fn installed(cfg: &Database) -> bool {
    cmd_ok("sh", &["-c", &format!("command -v {} >/dev/null 2>&1", shq(&cfg.bin))])
}

/// 数据库服务是否在运行。
pub fn server_running(_cfg: &Database) -> bool {
    cmd_ok("pgrep", &["-x", "mysqld"])
}

/// 清洗标识符：只保留小写字母/数字/下划线，杜绝注入。
pub fn sani(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// 执行单条 SQL，返回 (成功, 输出文本)。
fn run_sql(cfg: &Database, sql: &str) -> (bool, String) {
    let sql = format!("{};", sql.trim().trim_end_matches(';'));
    let mut cmd = std::process::Command::new(&cfg.bin);
    cmd.args(["--batch", "--skip-column-names"]);
    if !cfg.password.is_empty() {
        cmd.env("MYSQL_PWD", &cfg.password);
    }
    cmd.arg(format!("-u{}", cfg.user)).arg("-e").arg(&sql);
    run_cmd(&mut cmd)
}

/// 执行命令并返回 (成功, stdout 首行/错误信息)。
fn run_cmd(cmd: &mut std::process::Command) -> (bool, String) {
    match cmd.output() {
        Ok(o) if o.status.success() => {
            let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
            (true, out)
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr)
                .lines().next().unwrap_or("执行失败").trim().to_string();
            (false, err)
        }
        Err(e) => (false, e.to_string()),
    }
}

fn cmd_ok(bin: &str, args: &[&str]) -> bool {
    std::process::Command::new(bin).args(args).status().map(|s| s.success()).unwrap_or(false)
}

/// 执行 shell 命令，返回 (成功, 输出)。
fn run_shell(script: &str) -> (bool, String) {
    let mut cmd = std::process::Command::new("bash");
    cmd.arg("-c").arg(script);
    run_cmd(&mut cmd)
}

/// 列出所有数据库（排除系统库）。
pub fn databases(cfg: &Database) -> (bool, String) {
    let (ok, out) = run_sql(cfg, "SHOW DATABASES WHERE `Database` NOT IN ('information_schema','mysql','performance_schema','sys')");
    if !ok { return (false, out); }
    let list: Vec<&str> = out.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    let json = serde_json::json!(list).to_string();
    (true, json)
}

/// 列出所有账号 host/user。
pub fn users(cfg: &Database) -> (bool, String) {
    let (ok, out) = run_sql(cfg, "SELECT User,Host FROM mysql.user WHERE User NOT IN ('mysql.sys','root') ORDER BY User");
    if !ok { return (false, out); }
    let arr: Vec<serde_json::Value> = out.lines().filter_map(|l| {
        let mut it = l.split('\t');
        let u = it.next()?;
        let h = it.next().unwrap_or("localhost");
        Some(serde_json::json!({"user": u, "host": h}))
    }).collect();
    (true, serde_json::json!(arr).to_string())
}

/// 建库，可选字符集。
pub fn create_db(cfg: &Database, name: &str, charset: &str) -> (bool, String) {
    let n = sani(name);
    if n.is_empty() { return (false, "非法数据库名".into()); }
    let cs = if charset.trim().is_empty() { "utf8mb4" } else { charset.trim() };
    run_sql(cfg, &format!("CREATE DATABASE IF NOT EXISTS `{n}` DEFAULT CHARACTER SET {cs}"))
}

/// 删除数据库。
pub fn drop_db(cfg: &Database, name: &str) -> (bool, String) {
    let n = sani(name);
    if n.is_empty() { return (false, "非法数据库名".into()); }
    run_sql(cfg, &format!("DROP DATABASE IF EXISTS `{n}`"))
}

/// 建账号。
pub fn create_user(cfg: &Database, user: &str, pass: &str, host: &str) -> (bool, String) {
    let u = sani(user);
    let h = if host.trim().is_empty() { "localhost".into() } else { sani(host) };
    if u.is_empty() { return (false, "非法用户名".into()); }
    if h.is_empty() { return (false, "非法 host".into()); }
    // 密码仅过滤危险字符
    let p: String = pass.trim().chars().filter(|c| !matches!(c, '\'' | ';')).collect();
    let (ok, msg) = run_sql(cfg, &format!("CREATE USER IF NOT EXISTS '{u}'@'{h}' IDENTIFIED BY '{p}'"));
    if !ok { return (ok, msg); }
    run_sql(cfg, "FLUSH PRIVILEGES")
}

/// 授权：将某库的所有权限授予账号。
pub fn grant(cfg: &Database, db: &str, user: &str, host: &str) -> (bool, String) {
    let d = sani(db);
    let u = sani(user);
    let h = if host.trim().is_empty() { "localhost".into() } else { sani(host) };
    if d.is_empty() || u.is_empty() || h.is_empty() { return (false, "参数不合法".into()); }
    let r = run_sql(cfg, &format!("GRANT ALL PRIVILEGES ON `{d}`.* TO '{u}'@'{h}'"));
    let _ = run_sql(cfg, "FLUSH PRIVILEGES");
    r
}

/// 删除账号。
pub fn drop_user(cfg: &Database, user: &str, host: &str) -> (bool, String) {
    let u = sani(user);
    let h = if host.trim().is_empty() { "localhost".into() } else { sani(host) };
    if u.is_empty() || h.is_empty() { return (false, "参数不合法".into()); }
    run_sql(cfg, &format!("DROP USER IF EXISTS '{u}'@'{h}'"))
}

/// 备份指定数据库到 `dest_dir/<db>_<时间戳>.sql.gz`。
pub fn backup(cfg: &Database, db: &str, dest_dir: &str) -> (bool, String) {
    if !installed(cfg) { return (false, "未检测到 MySQL 客户端".into()); }
    let d = sani(db);
    if d.is_empty() { return (false, "非法数据库名".into()); }
    let _ = std::fs::create_dir_all(dest_dir);
    let ts = now_stamp();
    let gz = format!("{}/{}_{}.sql.gz", dest_dir.trim_end_matches('/'), d, ts);
    let script = format!(
        "MYSQL_PWD={} {} -u{} {} 2>/dev/null | gzip -c > {}",
        shq(&cfg.password), shq(&cfg.dump), cfg.user, shq(&d), shq(&gz),
    );
    let (ok, _) = run_shell(&script);
    if ok { (true, format!("已备份到 {}", gz)) } else { (false, "备份失败".into()) }
}

/// 恢复：把 SQL（含 gzip 压缩）导入指定库。
pub fn restore(cfg: &Database, db: &str, file: &str) -> (bool, String) {
    let d = sani(db);
    if d.is_empty() { return (false, "非法数据库名".into()); }
    let load = if file.ends_with(".gz") {
        format!("gzip -dc {} | {} -u{} {}", shq(file), shq(&cfg.bin), cfg.user, shq(&d))
    } else {
        format!("{} -u{} {} < {}", shq(&cfg.bin), cfg.user, shq(&d), shq(file))
    };
    run_shell(&format!("MYSQL_PWD={} bash -c 'set -o pipefail; {}'", shq(&cfg.password), load))
}

/// 备份文件列表 JSON。
pub fn backups_json(cfg: &Database, dir: &str) -> String {
    let mut files = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".sql.gz") { continue; }
            if let Ok(md) = e.metadata() {
                let size = md.len();
                let mtime = md.modified().ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()).unwrap_or(0);
                files.push(serde_json::json!({"name": name, "size": size, "time": mtime}));
            }
        }
    }
    files.sort_by(|a, b| b["time"].as_u64().unwrap_or(0).cmp(&a["time"].as_u64().unwrap_or(0)));
    serde_json::json!({"ok": true, "list": files}).to_string()
}

/// 重置 MySQL 的 root 用户密码。
pub fn reset_root_password(cfg: &Database, new_password: &str) -> (bool, String) {
    let npw = new_password.trim();
    if npw.len() < 4 { return (false, "新密码至少 4 位".into()); }
    run_shell(&format!(
        "MYSQL_PWD={} {} -u{} -e \"ALTER USER 'root'@'localhost' IDENTIFIED BY '{}'\"",
        shq(&cfg.password), shq(&cfg.bin), cfg.user, npw,
    ))
}

/// 当前时间戳（YYYYmmdd_HHMMSS）。
fn now_stamp() -> String {
    if let Ok(o) = std::process::Command::new("date").arg("+%Y%m%d_%H%M%S").output() {
        if let Ok(s) = String::from_utf8(o.stdout) {
            let t = s.trim().to_string();
            if !t.is_empty() { return t; }
        }
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}

/// shell 单引号转义。
fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
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
        assert_eq!(shq("a'b"), "'a'\\''b'");
    }

    #[test]
    fn now_stamp_nonempty() {
        assert!(!now_stamp().is_empty());
    }
}