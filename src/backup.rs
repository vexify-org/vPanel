//! 备份模块：
//!   - 目录备份：把任意目录打包为 `<src>-<ts>.tar.gz`；
//!   - 数据库备份：复用 db.rs 对全部库做 mysqldump；
//!   - 轮转：每个备份源保留最近 N 份，超出自动删除最旧的；
//!   - 计划任务：把「panel backup」写入 crontab 实现定时全量备份。

use crate::config::Config;
use crate::json;

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs()).unwrap_or(0)
}

fn strip_leading_slash(p: &str) -> String {
    p.trim_start_matches('/').replace('/', "_")
}

/// 目录备份：把 `src` 打包为 `<dest>/<basename>-<ts>.tar.gz`，然后轮转。
pub fn dir_backup(src: &str, dest: &str, keep: u32) -> (bool, String) {
    if src.trim().is_empty() { return (false, "缺少备份源路径".into()); }
    if !std::path::Path::new(src).exists() { return (false, format!("备份源不存在: {}", src)); }
    let keep = if keep == 0 { 5 } else { keep };
    let _ = std::fs::create_dir_all(dest);
    let base = std::path::Path::new(src)
        .file_name()
        .map(|x| x.to_string_lossy().into_owned())
        .unwrap_or_else(|| strip_leading_slash(src));
    let dst = format!("{}/{}-{}.tar.gz", dest.trim_end_matches('/'), base, now_ts());

    match std::process::Command::new("tar").args(["czf", &dst, "-C", src, "."]).output() {
        Ok(o) if o.status.success() => {
            rotate_prefix(dest.trim_end_matches('/'), &base, keep);
            (true, format!("已备份 {} -> {}", src, dst))
        }
        Ok(o) => {
            let _ = std::fs::remove_file(&dst);
            (false, format!("打包失败：{}", String::from_utf8_lossy(&o.stderr).trim()))
        }
        Err(e) => (false, format!("打包失败：{}", e)),
    }
}

/// 轮转：删除 dest 下以 `prefix-` 开头的最旧文件，直到保留 keep 份。
pub fn rotate_prefix(dest: &str, prefix: &str, keep: u32) {
    let mut files: Vec<(u64, std::path::PathBuf)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dest) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !(name.starts_with(&format!("{}-", prefix)) || name.starts_with(&format!("{}_", prefix))) {
                continue;
            }
            if !(name.ends_with(".tar.gz") || name.ends_with(".sql.gz")) { continue; }
            if let Ok(md) = e.metadata() {
                let mtime = md.modified().ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()).unwrap_or(0);
                files.push((mtime, e.path()));
            }
        }
    }
    files.sort_by_key(|f| f.0);
    while files.len() > keep as usize {
        let (_, path) = files.remove(0);
        let _ = std::fs::remove_file(path);
    }
}

/// 全量备份：目录（面板站点目录）+ 全部数据库。供定时任务调用。
pub fn run(cfg: &Config) -> (bool, String) {
    let mut msgs = Vec::new();
    let dir_src = crate::config::Config::panel_dir();
    let dir_dst = cfg.backup.dir.trim_end_matches('/');
    let (ok, msg) = dir_backup(&dir_src, dir_dst, cfg.backup.keep);
    msgs.push(format!("{} {}", if ok { "[ok]" } else { "[!!]" }, msg));

    if crate::db::installed(&cfg.database) {
        let (ok, dblist) = crate::db::databases(&cfg.database);
        if ok && !dblist.trim().is_empty() {
            if let Ok(arr) = serde_json::from_str::<Vec<String>>(&dblist) {
                for db in arr {
                    if db.starts_with("information_schema") || db.starts_with("performance_schema")
                        || db.starts_with("mysql") || db.starts_with("sys") { continue; }
                    let sub = format!("{}/db", cfg.backup.dir.trim_end_matches('/'));
                    let (ok, msg) = crate::db::backup(&cfg.database, &db, &sub);
                    rotate_prefix(&sub, &db, cfg.backup.keep);
                    msgs.push(format!("{} {}", if ok { "[ok]" } else { "[!!]" }, msg));
                }
            }
        }
    }
    (true, msgs.join("\n"))
}

/// 备份列表 -> JSON。
pub fn list_json(cfg: &Config) -> String {
    let dir = cfg.backup.dir.trim_end_matches('/');
    let files = collect_backups(dir);
    serde_json::json!({"ok": true, "dir": dir, "keep": cfg.backup.keep, "files": files}).to_string()
}

/// 递归收集备份文件，带 size/mtime，按新旧降序。
fn collect_backups(root: &str) -> Vec<serde_json::Value> {
    let mut files = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                files.extend(collect_backups(&p.to_string_lossy()));
            } else if name.ends_with(".gz") || name.ends_with(".tar.gz") {
                if let Ok(md) = e.metadata() {
                    let size = md.len();
                    let mtime = md.modified().ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs()).unwrap_or(0);
                    files.push(serde_json::json!({"name": name, "size": size, "mtime": mtime}));
                }
            }
        }
    }
    files.sort_by(|a, b| b["mtime"].as_u64().unwrap_or(0).cmp(&a["mtime"].as_u64().unwrap_or(0)));
    files
}

/// 安装定时全量备份（crontab）。
pub fn schedule(cfg: &Config, cron: &str) -> (bool, String) {
    let cron = if cron.trim().is_empty() {
        if cfg.backup.cron.trim().is_empty() { "0 3 * * *".into() } else { cfg.backup.cron.clone() }
    } else { cron.trim().to_string() };
    if cron.split_whitespace().count() != 5 { return (false, "cron 需为 5 段表达式".into()); }
    let exe = match std::env::current_exe() {
        Ok(e) => e.to_string_lossy().into_owned(),
        Err(_) => return (false, "无法定位面板可执行文件".into()),
    };
    let cfg_name = ["panel.yml", "panel.yaml", "config.yml", "config.yaml"]
        .iter().find(|n| std::path::Path::new(n).is_file()).map(|s| s.to_string());
    let mut cmd = exe;
    if let Some(c) = cfg_name { cmd.push(' '); cmd.push_str(&c); }
    cmd.push_str(" backup >/dev/null 2>&1");
    crate::ctl::task_add(&cron, &cmd)
}

/// 云备份上传：把本地备份文件上传到远程 FTP/FTPS 服务器。
pub fn cloud_upload(file: &str) -> (bool, String) {
    let host = std::env::var("VPANEL_FTP_HOST").unwrap_or_default();
    if host.trim().is_empty() { return (false, "未配置云存储（请设置 VPANEL_FTP_HOST）".into()); }
    if !std::path::Path::new(file).exists() { return (false, format!("待上传文件不存在: {}", file)); }
    let user = std::env::var("VPANEL_FTP_USER").unwrap_or_else(|_| "anonymous".into());
    let pass = std::env::var("VPANEL_FTP_PASS").unwrap_or_default();
    let dir = std::env::var("VPANEL_FTP_DIR").unwrap_or_else(|_| "/".into());
    let fname = std::path::Path::new(file).file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
    let remote = format!("{}/{}", dir.trim_end_matches('/'), fname);
    let script = format!(
        "lftp --env-password --ftp-pasv -u {} {} -e \"quote EXPOSE; put {} -o {}; bye\" <<< 'export FTP_PASSWORD={}' 2>&1",
        shq(&user), host, shq(file), shq(&remote), shq(&pass),
    );
    match std::process::Command::new("bash").arg("-c").arg(&script).output() {
        Ok(o) if o.status.success() => (true, format!("备份已上传到 {}:{}", host, remote)),
        Ok(o) => (false, format!("上传失败：{}", String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) => (false, e.to_string()),
    }
}

/// 从 crontab 移除本面板的定时备份。
pub fn schedule_remove() -> (bool, String) {
    let existing = std::process::Command::new("sh")
        .args(["-c", "crontab -l 2>/dev/null"]).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let kept: Vec<&str> = existing.lines().filter(|l| !l.contains("backup") || l.trim().is_empty()).collect();
    let new = kept.join("\n");
    let out = std::process::Command::new("bash").arg("-c").arg(format!("crontab - 2>/dev/null <<'EOF'\n{}\nEOF", new)).output();
    let ok = matches!(out, Ok(o) if o.status.success());
    (ok, if ok { "已移除定时备份任务".into() } else { "移除失败".into() })
}

fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basename_sanitizes() {
        assert_eq!(strip_leading_slash("/var/www"), "var_www");
        assert_eq!(strip_leading_slash("home/nginx"), "home_nginx");
    }

    #[test]
    fn rotate_prunes_oldest() {
        let dir = std::env::temp_dir().join("vp_bk_test");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        for i in 0..5 {
            let f = dir.join(format!("site-1000000{}.tar.gz", i));
            let _ = std::fs::write(&f, b"x");
        }
        rotate_prefix(dir.to_str().unwrap(), "site", 2);
        let left = std::fs::read_dir(&dir).map(|rd| rd.filter_map(|e| e.ok()).count()).unwrap_or(0);
        assert!(left <= 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}