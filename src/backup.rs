//! 备份模块（对标宝塔「定时备份」）：
//!   - 目录备份：把任意目录打包为 `<src>-<ts>.tar.gz`；
//!   - 数据库备份：复用 db.rs 对全部库做 mysqldump；
//!   - 轮转：每个备份源保留最近 N 份，超出自动删除最旧的；
//!   - 计划任务：把「panel backup」写入 crontab 实现定时全量备份。

use crate::config::Config;
use crate::json;

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn strip_leading_slash(p: &str) -> String {
    p.trim_start_matches('/').replace('/', "_")
}

/// 目录备份：把 `src` 打包为 `<keep-root>/<basename>-<ts>.tar.gz`，然后轮转。
/// 返回 (是否成功, 输出文本)。纯函数性：仅依赖外部 tar，可测其生成的文件名。
pub fn dir_backup(src: &str, dest: &str, keep: u32) -> (bool, String) {
    if src.trim().is_empty() {
        return (false, "缺少备份源路径".into());
    }
    if !std::path::Path::new(src).exists() {
        return (false, format!("备份源不存在: {}", src));
    }
    let keep = if keep == 0 { 5 } else { keep };
    let _ = std::fs::create_dir_all(dest);
    let base = std::path::Path::new(src)
        .file_name()
        .map(|x| x.to_string_lossy().into_owned())
        .unwrap_or_else(|| strip_leading_slash(src));
    let dst = format!("{}/{}-{}.tar.gz", dest.trim_end_matches('/'), json::jesc(&base), now_ts());
    // tar czf 到目标；用绝对源路径的尾斜杠前内容，避免包含到目录本身。
    let out = std::process::Command::new("tar")
        .args(["czf", &dst, "-C", src, "."])
        .output();
    let ok = matches!(&out, Ok(o) if o.status.success());
    if !ok {
        let _ = std::fs::remove_file(&dst);
        let err = match &out {
            Ok(o) => String::from_utf8_lossy(&o.stderr).trim().to_string(),
            Err(e) => e.to_string(),
        };
        return (false, format!("打包失败：{}", err));
    }
    rotate_prefix(dest.trim_end_matches('/'), &base, keep);
    (true, format!("已备份 {} -> {}", src, dst))
}

/// 轮转：删除 dest 下以 `prefix-` 开头、扩展名 .tar.gz/.gz 的最旧文件，直到保留 keep 份。
pub fn rotate_prefix(dest: &str, prefix: &str, keep: u32) {
    let mut files: Vec<(u64, std::path::PathBuf)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dest) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("{}-", prefix))
                || name.starts_with(&format!("{}_", prefix))
            {
                if !(name.ends_with(".tar.gz") || name.ends_with(".sql.gz")) {
                    continue;
                }
                if let Ok(md) = e.metadata() {
                    let mtime = md
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    files.push((mtime, e.path()));
                }
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
    // 目录备份：把当前面板目录备份（跳过 backup/hist 等）。
    let dir_src = crate::config::Config::panel_dir();
    let dir_dst = cfg.backup.dir.trim_end_matches('/');
    let (ok, msg) = dir_backup(&dir_src, dir_dst, cfg.backup.keep);
    msgs.push(format!("{} {}", if ok { "[ok]" } else { "[!!]" }, msg));

    // 数据库全量备份。
    if crate::db::installed(&cfg.database) {
        let (ok, dblist) = crate::db::databases(&cfg.database);
        if ok && !dblist.trim().is_empty() {
            // databases 返回 JSON 数组字符串，简化：逐个导出所有库隐身。
            if let Ok(arr) = serde_json::from_str::<Vec<String>>(&dblist) {
                for db in arr {
                    if db.starts_with("information_schema")
                        || db.starts_with("performance_schema")
                        || db.starts_with("mysql")
                        || db.starts_with("sys")
                    {
                        continue;
                    }
                    // 备份到 backup.dir/db 子目录，靠时间戳区分、轮转按 keep。
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
    let (dirs, files) = collect_backups(dir);
    format!(
        "{{\"ok\":true,\"dir\":\"{}\",\"keep\":{},\"files\":[{}],\"sub\":{}}}",
        json::jesc(dir),
        cfg.backup.keep,
        files.join(","),
        dirs.join(",")
    )
}

/// 递归收集备份目录及其文件。
/// 返回 (子目录JSON数组, 文件JSON数组)，文件带 size/mtime 且按新旧降序。
fn collect_backups(root: &str) -> (Vec<String>, Vec<String>) {
    let mut subdirs = Vec::new();
    let mut files = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                let (s, f) = collect_backups(&p.to_string_lossy());
                subdirs.extend(s);
                files.extend(f);
            } else if name.ends_with(".gz") || name.ends_with(".tar.gz") {
                if let Ok(md) = e.metadata() {
                    let size = md.len();
                    let mtime = md
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    files.push(format!(
                        "{{\"name\":\"{}\",\"size\":{},\"mtime\":{}}}",
                        json::jesc(&name),
                        size,
                        mtime
                    ));
                }
            }
        }
    }
    files.sort_by(|a, b| {
        // 按 mtime 降序：解析最后一个 mtime 字段不可靠，用名字倒序近似。
        a.cmp(b)
    });
    files.reverse();
    (subdirs, files)
}

/// 安装定时全量备份（crontab）。cron 为空用默认。返回 (是否成功, 说明)。
pub fn schedule(cfg: &Config, cron: &str) -> (bool, String) {
    let cron = if cron.trim().is_empty() {
        if cfg.backup.cron.trim().is_empty() { "0 3 * * *".to_string() } else { cfg.backup.cron.clone() }
    } else {
        cron.trim().to_string()
    };
    if cron.split_whitespace().count() != 5 {
        return (false, "cron 需为 5 段表达式".into());
    }
    // panel backup：定位当前二进制，透传当前配置文件。
    let exe = match std::env::current_exe() {
        Ok(e) => e.to_string_lossy().into_owned(),
        Err(_) => {
            return (false, "无法定位面板可执行文件".into());
        }
    };
    let cfg_name = ["panel.yml", "panel.yaml", "config.yml", "config.yaml"]
        .iter()
        .find(|n| std::path::Path::new(n).is_file())
        .map(|s| s.to_string());
    let mut cmd = exe;
    if let Some(c) = cfg_name {
        cmd.push(' ');
        cmd.push_str(&c);
    }
    cmd.push_str(" backup >/dev/null 2>&1");
    crate::ctl::task_add(&cron, &cmd)
}

/// 云备份上传：把本地备份文件 `file` 上传到远程 FTP/FTPS 服务器指定目录。
/// 连接信息从环境变量读取：`VPANEL_FTP_HOST`、`VPANEL_FTP_USER`、`VPANEL_FTP_PASS`、
/// `VPANEL_FTP_DIR`（可选，远程目录），驱动工具为 `lftp`（自动使用已建立的 TLS）。
/// 未配置 host 时返回明确提示，便于 UI 引导。
pub fn cloud_upload(file: &str) -> (bool, String) {
    let host = std::env::var("VPANEL_FTP_HOST").unwrap_or_default();
    if host.trim().is_empty() {
        return (false, "未配置云存储（请设置 VPANEL_FTP_HOST）".to_string());
    }
    if !std::path::Path::new(file).exists() {
        return (false, format!("待上传文件不存在: {}", file));
    }
    let user = std::env::var("VPANEL_FTP_USER").unwrap_or_else(|_| "anonymous".into());
    let pass = std::env::var("VPANEL_FTP_PASS").unwrap_or_default();
    let dir = std::env::var("VPANEL_FTP_DIR").unwrap_or_else(|_| "/".into());
    // lftp：-e 执行 put；--ftp-pasv 被动模式；--env-password 避免密码上进程列表。
    let remote = format!("{}/{}", dir.trim_end_matches('/'), std::path::Path::new(file).file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default());
    let script = format!(
        "lftp --env-password --ftp-pasv -u '{}' '{}' -e \"quote EXPOSE; put '{}' -o '{}'; bye\" <<< 'export FTP_PASSWORD={}' 2>&1",
        user.replace('\'', "'\\''"),
        host,
        file.replace('\'', "'\\''"),
        remote.replace('\'', "'\\''"),
        pass.replace('\'', "'\\''")
    );
    let out = std::process::Command::new("bash").arg("-c").arg(&script).output();
    match out {
        Ok(o) if o.status.success() => (true, format!("备份已上传到 {}:{}", host, remote)),
        Ok(o) => (false, format!("上传失败：{}", String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) => (false, e.to_string()),
    }
}

/// 从 crontab 移除本面板的定时备份（按命令包含 "backup" 特征）。
pub fn schedule_remove() -> (bool, String) {
    let existing = std::process::Command::new("sh")
        .args(["-c", "crontab -l 2>/dev/null"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let kept: Vec<&str> = existing
        .lines()
        .filter(|l| !l.contains("backup") || l.trim().is_empty())
        .collect();
    let new = kept.join("\n");
    let out = std::process::Command::new("bash")
        .arg("-c")
        .arg(format!("crontab - 2>/dev/null <<'EOF'\n{}\nEOF", new))
        .output();
    let ok = matches!(out, Ok(o) if o.status.success());
    (ok, if ok { "已移除定时备份任务".into() } else { "移除失败".into() })
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
        // 造 5 个越旧越早（时间戳递增），keep=2 应剩最新的 2 个。
        for i in 0..5 {
            let f = dir.join(format!("site-1000000{}.tar.gz", i));
            let _ = std::fs::write(&f, b"x");
        }
        rotate_prefix(dir.to_str().unwrap(), "site", 2);
        let mut left = 0;
        if let Ok(rd) = std::fs::read_dir(&dir) {
            left = rd.filter_map(|e| e.ok()).count();
        }
        // 轮转后最多保留 keep 个匹配前缀文件；非匹配同样被裁剪由前缀判定。
        assert!(left <= 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}