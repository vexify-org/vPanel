//! 运维工具箱（对标宝塔/类三方的批量工具）——全部「按需执行、随求即释」，
//! 只封装系统命令 / 读 /proc，不新增常驻状态，内存维持有界。
//!
//! 每个函数都是独立纯函数：入参即结果，供 `/mcp` 与 `/api/*` 复用。

use crate::json;

/// 只读命令：stdout，成功才返回。
fn cmd(c: &str) -> Option<String> {
    let out = std::process::Command::new("/bin/sh").arg("-c").arg(c).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        None
    }
}

/// 只读命令，失败也返回（用于诊断含 exit code）。
fn cmd_e(c: &str) -> (String, String) {
    match std::process::Command::new("/bin/sh").arg("-c").arg(c).output() {
        Ok(o) => (
            String::from_utf8_lossy(&o.stdout).trim_end().to_string(),
            String::from_utf8_lossy(&o.stderr).trim_end().to_string(),
        ),
        Err(e) => (String::new(), e.to_string()),
    }
}

fn pfile(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

// ---------------------------------------------------------------------------
// 一、网络诊断
// ---------------------------------------------------------------------------

/// ping 指定主机 / IP。
pub fn ping(host: &str, count: u32) -> String {
    let count = if count == 0 { 4 } else { count.min(20) };
    match cmd_e(&format!("ping -c {} -W 2 {} 2>&1", count, shq(host))) {
        (o, _) if !o.is_empty() => format!("{{\"ok\":true,\"host\":\"{}\",\"out\":\"{}\"}}", json::jesc(host), json::jesc(&o)),
        (_, e) => format!("{{\"ok\":false,\"host\":\"{}\",\"msg\":\"{}\"}}", json::jesc(host), json::jesc(&e)),
    }
}

/// TCP 通断（负责探测端口是否可达），返回结果行。
pub fn tcp_ping(host: &str, port: u32, count: u32) -> String {
    let port = if port == 0 { 80 } else { port };
    let n = if count == 0 { 3 } else { count.min(10) };
    // 用 time 与 nc 兜底；优先 nc -z。
    let out = cmd("command -v nc >/dev/null 2>&1 || command -v ncat >/dev/null 2>&1")
        .map(|_| "found")
        .unwrap_or_default();
    let (o, e, ok) = if !out.is_empty() {
        let mut succ = 0u32;
        let mut detail = Vec::new();
        for _ in 0..n {
            let r = cmd(&format!("nc -z -w 2 {} {}", host, port));
            if r.is_some() { succ += 1; detail.push("ok"); } else { detail.push("fail"); }
        }
        (format!("{}/{} reachable ({})", succ, n, detail.join(",")), String::new(), succ > 0)
    } else {
        let (o, e) = cmd_e(&format!("timeout 2 bash -c 'exec 3<>/dev/tcp/{}/{}' 2>&1", host, port));
        let ok = o.is_empty();
        (o, e, ok)
    };
    format!("{{\"ok\":{},\"host\":\"{}\",\"port\":{},\"out\":\"{}\",\"err\":\"{}\"}}",
        ok, json::jesc(host), port, json::jesc(&o), json::jesc(&e))
}

/// DNS 解析查询。
pub fn dns_lookup(host: &str) -> String {
    let (o, e) = cmd_e(&format!("getent hosts {} 2>&1 || nslookup {} 2>&1 || dig +short {} 2>&1", shq(host), shq(host), shq(host)));
    let ok = !o.trim().is_empty();
    format!("{{\"ok\":{},\"host\":\"{}\",\"out\":\"{}\",\"err\":\"{}\"}}", ok, json::jesc(host), json::jesc(&o), json::jesc(&e))
}

/// HTTP 响应头探测（curl -sI）。
pub fn http_head(url: &str) -> String {
    let (o, e) = cmd_e(&format!("curl -sI --max-time 8 {} 2>&1", shq(url)));
    let ok = !o.trim().is_empty();
    format!("{{\"ok\":{},\"url\":\"{}\",\"out\":\"{}\",\"err\":\"{}\"}}", ok, json::jesc(url), json::jesc(&o), json::jesc(&e))
}

/// 本机所有监听端口（ss -tlnp）。
pub fn listener_ports() -> String {
    let (o, e) = cmd_e("ss -tlnp 2>/dev/null || netstat -tlnp 2>/dev/null");
    let ok = !o.is_empty();
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", ok, json::jesc(&o), json::jesc(&e))
}

/// 指定端口是否在监听。
pub fn port_check(port: u32) -> String {
    let (o, e) = cmd_e(&format!("ss -tln 'sport = :{}' 2>/dev/null || netstat -tlnp 2>/dev/null | grep ':{} '", port, port));
    let ok = o.contains(&format!(":{}", port));
    format!("{{\"ok\":{},\"port\":{},\"out\":\"{}\",\"err\":\"{}\"}}", ok, port, json::jesc(&o), json::jesc(&e))
}

/// 反向解析主机名。
pub fn reverse_dns(ip: &str) -> String {
    let (o, e) = cmd_e(&format!("getent hosts {} 2>&1 || host {} 2>&1", shq(ip), shq(ip)));
    let ok = !o.trim().is_empty();
    format!("{{\"ok\":{},\"ip\":\"{}\",\"out\":\"{}\",\"err\":\"{}\"}}", ok, json::jesc(ip), json::jesc(&o), json::jesc(&e))
}

// ---------------------------------------------------------------------------
// 二、系统纵深
// ---------------------------------------------------------------------------

/// CPU 型号与核心数。
pub fn cpu() -> String {
    let (cores, model) = crate::system::cpu_info();
    format!("{{\"ok\":true,\"cores\":{},\"model\":\"{}\"}}", cores, json::jesc(&model))
}

/// 当前 CPU 使用率（采样 1 秒）。
pub fn cpu_usage() -> String {
    let (a_idle, a_total) = cpu_sample();
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let (b_idle, b_total) = cpu_sample();
    if a_total == b_total {
        return "{\"ok\":true,\"usage_pct\":0}".to_string();
    }
    let busy = (b_total - b_idle).saturating_sub(a_total - a_idle);
    let pct = busy as f64 / (b_total - a_total) as f64 * 100.0;
    format!("{{\"ok\":true,\"usage_pct\":{:.1}}}", pct)
}

fn cpu_sample() -> (u64, u64) {
    let mut idle = 0u64;
    let mut total = 0u64;
    if let Some(s) = pfile("/proc/stat") {
        if let Some(line) = s.lines().find(|l| l.starts_with("cpu ")) {
            for (i, v) in line.split_whitespace().skip(1).enumerate() {
                if let Ok(v) = v.parse::<u64>() {
                    total += v;
                    if i == 3 || i == 4 { idle += v; }
                }
            }
        }
    }
    (idle, total)
}

/// 内存信息（总/可用/已用）。
pub fn mem_info() -> String {
    let (total, avail) = crate::system::mem().unwrap_or((0, 0));
    let used = total.saturating_sub(avail);
    format!("{{\"ok\":true,\"total\":{},\"used\":{},\"avail\":{}}}", total, used, avail)
}

/// 交换分区。
pub fn swap_info() -> String {
    let mut total = 0u64;
    let mut free = 0u64;
    if let Some(s) = pfile("/proc/meminfo") {
        for l in s.lines() {
            if let Some(v) = l.strip_prefix("SwapTotal:") {
                total = v.trim().split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0) * 1024;
            } else if let Some(v) = l.strip_prefix("SwapFree:") {
                free = v.trim().split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0) * 1024;
            }
        }
    }
    format!("{{\"ok\":true,\"total\":{},\"used\":{},\"free\":{}}}", total, total.saturating_sub(free), free)
}

/// 系统负载（1/5/15 分钟）。
pub fn loadavg() -> String {
    let s = pfile("/proc/loadavg").unwrap_or_default();
    let parts: Vec<&str> = s.split_whitespace().take(3).collect();
    let l1 = parts.first().copied().unwrap_or("0");
    let l5 = parts.get(1).copied().unwrap_or("0");
    let l15 = parts.get(2).copied().unwrap_or("0");
    format!("{{\"ok\":true,\"load1\":\"{}\",\"load5\":\"{}\",\"load15\":\"{}\"}}", l1, l5, l15)
}

/// 网络吞吐（全网接口合计 KB/s，采样 1 秒）。
pub fn net_io() -> String {
    let a = net_bytes();
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let b = net_bytes();
    let rx = (b.0.saturating_sub(a.0)) as f64 / 1024.0;
    let tx = (b.1.saturating_sub(a.1)) as f64 / 1024.0;
    format!("{{\"ok\":true,\"rx_kbs\":{:.1},\"tx_kbs\":{:.1}}}", rx, tx)
}

fn net_bytes() -> (u64, u64) {
    let mut rx = 0u64;
    let mut tx = 0u64;
    if let Some(s) = pfile("/proc/net/dev") {
        for line in s.lines().skip(2) {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() >= 10 {
                if let (Ok(r), Ok(t)) = (f[1].parse::<u64>(), f[9].parse::<u64>()) {
                    rx += r;
                    tx += t;
                }
            }
        }
    }
    (rx, tx)
}

/// 磁盘 inode 使用。
pub fn disk_inodes() -> String {
    let (o, e) = cmd_e("df -i 2>/dev/null");
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

/// 操作系统发行版名。
pub fn os_release() -> String {
    let s = pfile("/etc/os-release")
        .map(|x| x.lines().find(|l| l.starts_with("PRETTY_NAME"))
            .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string())
            .unwrap_or_default())
        .unwrap_or_default();
    format!("{{\"ok\":true,\"os\":\"{}\"}}", json::jesc(&s))
}

/// 内核版本、主机名、架构、运行时间。
pub fn kernel_info() -> String {
    let k = pfile("/proc/sys/kernel/osrelease").unwrap_or_default();
    let h = pfile("/proc/sys/kernel/hostname").unwrap_or_default();
    let up = pfile("/proc/uptime").and_then(|s| s.split_whitespace().next().map(|x| x.parse::<u64>().unwrap_or(0))).unwrap_or(0);
    let days = up / 86400;
    format!("{{\"ok\":true,\"kernel\":\"{}\",\"host\":\"{}\",\"arch\":\"{}\",\"uptime_days\":{}}}",
        json::jesc(&k), json::jesc(&h), std::env::consts::ARCH, days)
}

// ---------------------------------------------------------------------------
// 三、文件系统深度
// ---------------------------------------------------------------------------

/// 文件条目统计（权限/大小/属主/类型/时间）。
pub fn ls_long(path: &str) -> String {
    let (o, e) = cmd_e(&format!("ls -lha {} 2>&1", shq(path)));
    format!("{{\"ok\":{},\"path\":\"{}\",\"out\":\"{}\",\"err\":\"{}\"}}",
        !o.is_empty() || !path.starts_with('/'), json::jesc(path), json::jesc(&o), json::jesc(&e))
}

/// 目录占用总大小。
pub fn dir_size(path: &str) -> String {
    let (o, e) = cmd_e(&format!("du -sh {} 2>&1", shq(path)));
    let ok = !o.trim().is_empty();
    format!("{{\"ok\":{},\"path\":\"{}\",\"size\":\"{}\",\"err\":\"{}\"}}", ok, json::jesc(path), json::jesc(&o.split_whitespace().next().unwrap_or("")), json::jesc(&e))
}

/// 统计目录内文件数。
pub fn file_count(path: &str) -> String {
    let (o, _) = cmd_e(&format!("find {} -type f 2>/dev/null | wc -l", shq(path)));
    let n = o.trim().parse::<u64>().unwrap_or(0);
    format!("{{\"ok\":true,\"path\":\"{}\",\"files\":{}}}", json::jesc(path), n)
}

/// 递归搜索文件。
pub fn file_search(dir: &str, pattern: &str) -> String {
    let (o, e) = cmd_e(&format!("find {} -name '{}' 2>&1 | head -200", shq(dir), pattern.replace('\'', "'\\''")));
    format!("{{\"ok\":true,\"dir\":\"{}\",\"pattern\":\"{}\",\"out\":\"{}\",\"err\":\"{}\"}}",
        json::jesc(dir), json::jesc(pattern), json::jesc(&o), json::jesc(&e))
}

/// 修改权限 chmod（相对/绝对）。
pub fn file_chmod(path: &str, mode: &str) -> (bool, String) {
    if mode.trim().is_empty() || !mode.chars().all(|c| c.is_ascii_digit() || c == '+' || c == '-' || c == '=' || c.is_alphabetic()) {
        return (false, "权限格式非法".into());
    }
    let out = std::process::Command::new("/bin/sh").arg("-c").arg(format!("chmod {} {}", shq(mode), shq(path))).output();
    match out {
        Ok(o) if o.status.success() => (true, format!("已设置 {} 权限 -> {}", mode, path)),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// 打包为 tar.gz。
pub fn zip_archive(src: &str, dst: &str) -> (bool, String) {
    if !std::path::Path::new(src).exists() {
        return (false, format!("源不存在: {}", src));
    }
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("tar czf {} -C {} .", shq(dst), shq(src)))
        .output();
    match out {
        Ok(o) if o.status.success() => (true, format!("已打包 {} -> {}", src, dst)),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// 解压 tar.gz。
pub fn zip_extract(file: &str, dest: &str) -> (bool, String) {
    if !std::path::Path::new(file).exists() {
        return (false, format!("文件不存在: {}", file));
    }
    let _ = std::fs::create_dir_all(dest);
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("tar xzf {} -C {}", shq(file), shq(dest)))
        .output();
    match out {
        Ok(o) if o.status.success() => (true, format!("已解压 {} -> {}", file, dest)),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// 文件头部 N 行。
pub fn file_head(path: &str, n: usize) -> String {
    let n = if n == 0 { 20 } else { n.min(500) };
    let (o, e) = cmd_e(&format!("head -n {} {} 2>&1", n, shq(path)));
    format!("{{\"ok\":{},\"path\":\"{}\",\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(path), json::jesc(&o), json::jesc(&e))
}

/// 文件字节大小。
pub fn file_size(path: &str) -> String {
    match std::fs::metadata(path) {
        Ok(m) => format!("{{\"ok\":true,\"path\":\"{}\",\"size\":{}}}", json::jesc(path), m.len()),
        Err(e) => format!("{{\"ok\":false,\"path\":\"{}\",\"msg\":\"{}\"}}", json::jesc(path), json::jesc(&e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// 四、进程
// ---------------------------------------------------------------------------

/// 按名字查找进程（ps）。
pub fn process_by_name(name: &str) -> String {
    let (o, e) = cmd_e(&format!("pgrep -af '{}' 2>&1 || ps -eo pid,user,comm,args | grep '{}' | grep -v grep", name.replace('\'', "'\\''"), name.replace('\'', "'\\''")));
    format!("{{\"ok\":true,\"name\":\"{}\",\"out\":\"{}\",\"err\":\"{}\"}}", json::jesc(name), json::jesc(&o), json::jesc(&e))
}

/// 单进程详情（/proc/<pid>/status + cmdline）。
pub fn process_detail(pid: u32) -> String {
    if pid == 0 {
        return "{\"ok\":false,\"msg\":\"缺少 pid\"}".to_string();
    }
    let stat = pfile(&format!("/proc/{}/status", pid)).unwrap_or_default();
    let cmd = std::fs::read_to_string(format!("/proc/{}/cmdline", pid))
        .ok().map(|s| s.replace('\0', " ").trim().to_string()).unwrap_or_default();
    let ok = !stat.is_empty();
    format!("{{\"ok\":{},\"pid\":{},\"status\":\"{}\",\"cmdline\":\"{}\"}}",
        ok, pid, json::jesc(&stat), json::jesc(&cmd))
}

// ---------------------------------------------------------------------------
// 五、服务 / systemd
// ---------------------------------------------------------------------------

/// 列出 systemd 服务单元。
pub fn systemd_units() -> String {
    let (o, e) = cmd_e("systemctl list-units --type=service --no-pager 2>&1 | head -120");
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

/// 对 systemd 单元执行动作。
pub fn systemd_action(unit: &str, action: &str) -> (bool, String) {
    if !matches!(action, "start" | "stop" | "restart" | "reload" | "enable" | "disable") {
        return (false, "action 应为 start/stop/restart/reload/enable/disable".into());
    }
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("systemctl {} {}", action, shq(unit)))
        .output();
    match out {
        Ok(o) if o.status.success() => (true, format!("{} {} 成功", action, unit)),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// 六、软件包管理（apt）
// ---------------------------------------------------------------------------

pub fn apt_update() -> (bool, String) {
    run_pkg("apt-get update -y")
}

pub fn apt_upgrade() -> (bool, String) {
    run_pkg("apt-get upgrade -y")
}

pub fn apt_install(pkg: &str) -> (bool, String) {
    run_pkg(&format!("apt-get install -y {}", shq(pkg)))
}

pub fn apt_remove(pkg: &str) -> (bool, String) {
    run_pkg(&format!("apt-get remove -y {}", shq(pkg)))
}

fn run_pkg(c: &str) -> (bool, String) {
    let out = std::process::Command::new("/bin/sh").arg("-c").arg(c).output();
    match out {
        Ok(o) if o.status.success() => (true, "命令执行成功".into()),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().chars().take(300).collect()),
        Err(e) => (false, e.to_string()),
    }
}

/// 已安装软件列表（dpkg）。
pub fn apt_list_installed() -> String {
    let (o, e) = cmd_e("dpkg-query -W -f '${Package}|${Version}|${Status}\\n' 2>/dev/null | head -300");
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

/// 软件是否已安装。
pub fn pkg_installed(pkg: &str) -> String {
    let r = cmd(&format!("dpkg-query -W {} >/dev/null 2>&1", pkg)).map(|_| true).unwrap_or(false);
    format!("{{\"ok\":true,\"pkg\":\"{}\",\"installed\":{}}}", json::jesc(pkg), r)
}

// ---------------------------------------------------------------------------
// 七、计划任务（cron）
// ---------------------------------------------------------------------------

/// 列出系统 crontab。
pub fn cron_list() -> String {
    let (o, e) = cmd_e("crontab -l 2>&1");
    format!("{{\"ok\":true,\"out\":\"{}\",\"err\":\"{}\"}}", json::jesc(&o), json::jesc(&e))
}

/// 追加一行 cron 到当前用户 crontab。
pub fn cron_add(schedule: &str, command: &str) -> (bool, String) {
    if schedule.trim().is_empty() || command.trim().is_empty() {
        return (false, "缺少 schedule 或 command".into());
    }
    let line = format!("{} {}", schedule.trim(), command.trim());
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("(crontab -l 2>/dev/null; echo '{}') | crontab -", line.replace('\'', "'\\''")))
        .output();
    match out {
        Ok(o) if o.status.success() => (true, format!("已添加任务: {}", line)),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// 移除包含关键字的 cron 行。
pub fn cron_remove(keyword: &str) -> (bool, String) {
    if keyword.trim().is_empty() {
        return (false, "缺少 keyword".into());
    }
    let kw = keyword.replace('\'', "'\\''");
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("crontab -l 2>/dev/null | grep -v '{}' | crontab -", kw))
        .output();
    match out {
        Ok(o) if o.status.success() => (true, format!("已移除包含 [{}] 的任务", keyword)),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// 列出 /etc/cron.d 与系统 crontab。
pub fn cron_system() -> String {
    let (o, _) = cmd_e("cat /etc/crontab 2>/dev/null; ls /etc/cron.d/ 2>/dev/null");
    format!("{{\"ok\":true,\"out\":\"{}\"}}", json::jesc(&o))
}

// ---------------------------------------------------------------------------
// 八、运行时版本（PHP / Node / Go / Python / MySQL）
// ---------------------------------------------------------------------------

pub fn php_version() -> String {
    let (o, e) = cmd_e("php -v 2>&1 | head -2");
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

pub fn node_version() -> String {
    let (o, e) = cmd_e("node -v 2>&1");
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

pub fn go_version() -> String {
    let (o, e) = cmd_e("go version 2>&1");
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

pub fn python_version() -> String {
    let (o, e) = cmd_e("python3 --version 2>&1");
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

pub fn mysql_version() -> String {
    let (o, e) = cmd_e("mysql --version 2>&1");
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

/// 列出已安装的 PHP-FPM socket。
pub fn php_fpm_sockets() -> String {
    let (o, _) = cmd_e("ls /run/php/*.sock 2>/dev/null; which php-fpm* 2>/dev/null");
    format!("{{\"ok\":true,\"out\":\"{}\"}}", json::jesc(&o))
}

// ---------------------------------------------------------------------------
// 九、数据库深化
// ---------------------------------------------------------------------------

/// 各库大小（information_schema）。
pub fn db_sizes(cfg: &crate::config::Database) -> (bool, String) {
    let q = "SELECT table_schema,ROUND(SUM(data_length+index_length)/1024/1024,2) MB FROM information_schema.tables GROUP BY table_schema ORDER BY MB DESC";
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("MYSQL_PWD={} mysql -u {} -e '{}' 2>&1", shq(&cfg.password), cfg.user, q.replace('\'', "\\'")))
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
            (true, t)
        }
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// MySQL 运行状态（uptime / 线程 / 版本）。
pub fn mysql_status(cfg: &crate::config::Database) -> (bool, String) {
    let q = "SHOW GLOBAL STATUS WHERE Variable_name IN ('Uptime','Threads_connected','Threads_running','Connections'); SELECT VERSION() ver";
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("MYSQL_PWD={} mysql -u {} -e '{}' 2>&1", shq(&cfg.password), cfg.user, q.replace('\'', "\\'")))
        .output();
    match out {
        Ok(o) if o.status.success() => (true, String::from_utf8_lossy(&o.stdout).trim().to_string()),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// 检查 MySQL 是否可连通（SELECT 1）。
pub fn mysql_ping(cfg: &crate::config::Database) -> String {
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("MYSQL_PWD={} {} -u{} -e 'SELECT 1' 2>&1", shq(&cfg.password), shq(&cfg.bin), cfg.user))
        .output();
    let ok = matches!(&out, Ok(o) if o.status.success());
    format!("{{\"ok\":{},\"user\":\"{}\"}}", ok, json::jesc(&cfg.user))
}

// ---------------------------------------------------------------------------
// 十、SSL 深化
// ---------------------------------------------------------------------------

/// 查看证书明细（openssl x509 解析 PEM）。
pub fn cert_view(name: &str, cfg: &crate::config::Certs) -> (bool, String) {
    let path = format!("{}/{}.crt", cfg.dir, name.trim());
    if !std::path::Path::new(&path).exists() {
        return (false, format!("证书不存在: {}", name));
    }
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("openssl x509 -in {} -noout -subject -issuer -dates -serial 2>&1", shq(&path)))
        .output();
    match out {
        Ok(o) if o.status.success() => (true, String::from_utf8_lossy(&o.stdout).trim().to_string()),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// 证书剩余天数（shell 里用 date 算剩余天数）。
pub fn cert_expiry(name: &str, cfg: &crate::config::Certs) -> String {
    let path = format!("{}/{}.crt", cfg.dir, name.trim());
    if !std::path::Path::new(&path).exists() {
        return format!("{{\"ok\":false,\"name\":\"{}\",\"msg\":\"证书不存在\"}}", json::jesc(name));
    }
    let script = format!(
        "E=$(openssl x509 -in {} -noout -enddate 2>/dev/null | cut -d= -f2); N=$(date +%s); X=$(date -d \"$E\" +%s 2>/dev/null || echo 0); echo $(( (X-N)/86400 ))",
        shq(&path)
    );
    let (o, _) = cmd_e(&script);
    let days: i64 = o.trim().parse().unwrap_or(-1);
    format!("{{\"ok\":true,\"name\":\"{}\",\"days_left\":{}}}", json::jesc(name), days)
}

// ---------------------------------------------------------------------------
// 十一、Docker 深化
// ---------------------------------------------------------------------------

pub fn docker_images() -> String {
    let (o, e) = cmd_e("docker images --format '{{.Repository}}:{{.Tag}}|{{.Size}}' 2>&1");
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

pub fn docker_stats() -> String {
    let (o, e) = cmd_e("docker stats --no-stream --format '{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}' 2>&1");
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

pub fn docker_prune() -> (bool, String) {
    let out = std::process::Command::new("docker").args(["system", "prune", "-f"]).output();
    match out {
        Ok(o) if o.status.success() => (true, "已清理未使用的 Docker 资源".into()),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

pub fn docker_info_json() -> String {
    let (o, e) = cmd_e("docker version --format '{{.Server.Version}}' 2>&1");
    format!("{{\"ok\":{},\"version\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

// ---------------------------------------------------------------------------
// 十二、日志
// ---------------------------------------------------------------------------

pub fn dmesg_tail(n: usize) -> String {
    let n = if n == 0 { 30 } else { n.min(200) };
    let (o, e) = cmd_e(&format!("dmesg 2>/dev/null | tail -n {} || journalctl -k -n {} --no-pager 2>&1", n, n));
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

pub fn journal_tail(unit: &str, n: usize) -> String {
    let n = if n == 0 { 30 } else { n.min(200) };
    let (o, e) = cmd_e(&format!("journalctl -u {} -n {} --no-pager 2>&1", shq(unit), n));
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

pub fn nginx_error_tail(n: usize) -> String {
    log_tail_file("/var/log/nginx/error.log", n)
}

pub fn nginx_access_tail(n: usize) -> String {
    log_tail_file("/var/log/nginx/access.log", n)
}

pub fn mysql_error_tail(n: usize) -> String {
    log_tail_file("/var/log/mysql/error.log", n)
}

pub fn auth_log_tail(n: usize) -> String {
    let n = if n == 0 { 30 } else { n.min(200) };
    let (o, e) = cmd_e(&format!("tail -n {} /var/log/auth.log 2>&1", n));
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

fn log_tail_file(p: &str, n: usize) -> String {
    let n = if n == 0 { 30 } else { n.min(200) };
    let (o, e) = cmd_e(&format!("tail -n {} {} 2>&1", n, p));
    format!("{{\"ok\":{},\"path\":\"{}\",\"out\":\"{}\",\"err\":\"{}\"}}", std::path::Path::new(p).exists(), p, json::jesc(&o), json::jesc(&e))
}

// ---------------------------------------------------------------------------
// 十三、用户 / 杂项
// ---------------------------------------------------------------------------

pub fn users_list() -> String {
    let (o, e) = cmd_e("cut -d: -f1,3,7 /etc/passwd 2>&1");
    format!("{{\"ok\":{},\"out\":\"{}\",\"err\":\"{}\"}}", !o.is_empty(), json::jesc(&o), json::jesc(&e))
}

pub fn whoami() -> String {
    let (o, _) = cmd_e("id 2>&1");
    format!("{{\"ok\":true,\"out\":\"{}\"}}", json::jesc(&o))
}

pub fn random_password(len: u32) -> String {
    use rand::Rng;
    use rand::thread_rng;
    let len = (if len == 0 { 16 } else { len }).min(64) as usize;
    let charset = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#%^&*-_=+";
    let bytes = charset.as_bytes();
    let mut rng = thread_rng();
    let mut pw: Vec<u8> = (0..len).map(|_| bytes[rng.gen_range(0..bytes.len())]).collect();
    // 保证各大类都出现
    for set in ["abcdefghijklmnopqrstuvwxyz", "ABCDEFGHIJKLMNOPQRSTUVWXYZ", "0123456789", "!@#%^&*-_=+"] {
        if !pw.iter().any(|c| set.contains(*c as char)) {
            pw[0] = set.as_bytes()[rng.gen_range(0..set.len())];
        }
    }
    let s = String::from_utf8_lossy(&pw).into_owned();
    format!("{{\"ok\":true,\"password\":\"{}\"}}", json::jesc(&s))
}

pub fn sha256(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    let d = h.finalize();
    format!("{{\"ok\":true,\"sha256\":\"{:x}\"}}", d)
}

pub fn base64_encode(text: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    format!("{{\"ok\":true,\"base64\":\"{}\"}}", STANDARD.encode(text.as_bytes()))
}

pub fn base64_decode(enc: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    match STANDARD.decode(enc.trim()) {
        Ok(b) => format!("{{\"ok\":true,\"decoded\":\"{}\"}}", json::jesc(&String::from_utf8_lossy(&b))),
        Err(_) => "{\"ok\":false,\"msg\":\"base64 解码失败\"}".to_string(),
    }
}

/// 生成一个随机 v4 风格 UUID。
pub fn uuid() -> String {
    use rand::Rng;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    let mut r = StdRng::from_entropy();
    let mut b = [0u8; 16];
    for x in b.iter_mut() { *x = r.gen(); }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{{\"ok\":true,\"uuid\":\"{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}\"}}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// 面板自述：版本与内存预算。
pub fn panel_about() -> String {
    let (total, _) = crate::system::mem().unwrap_or((0, 0));
    let rss = rss_bytes();
    format!(
        "{{\"ok\":true,\"name\":\"vpanel\",\"version\":\"{}\",\"mem_total_mb\":{:.1},\"rss_mb\":{:.1}}}",
        env!("CARGO_PKG_VERSION"),
        total as f64 / 1048576.0,
        rss as f64 / 1048576.0
    )
}

fn rss_bytes() -> u64 {
    pfile("/proc/self/statm")
        .and_then(|s| s.split_whitespace().nth(1).map(|x| x.parse::<u64>().unwrap_or(0)))
        .map(|pages| pages * 4096)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 九、安全 / 网络 / 系统实用（新增，全部纯函数、无状态）
// ---------------------------------------------------------------------------

/// 防火墙规则（iptables -S；无 iptables 时退回 nft list ruleset）。
pub fn firewall_rules() -> String {
    let out = cmd("iptables -S 2>/dev/null || nft list ruleset 2>/dev/null");
    match out {
        Some(o) => format!("{{\"ok\":true,\"rules\":\"{}\"}}", json::jesc(&o)),
        None => "{\"ok\":false,\"msg\":\"无 iptables / nftables\"}".to_string(),
    }
}

/// 网卡地址表（ip -o addr）-> JSON 数组。
pub fn net_interfaces() -> String {
    let out = cmd("ip -o addr show 2>/dev/null");
    let mut first = true;
    let mut s = String::from("{\"ok\":true,\"ifaces\":[");
    if let Some(o) = out {
        for line in o.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 4 {
                continue;
            }
            let iface = f[1];
            let addrs = f[3];
            if !first {
                s.push(',');
            }
            first = false;
            s.push_str(&format!(
                "{{\"iface\":\"{}\",\"inet\":\"{}\"}}",
                json::jesc(iface),
                json::jesc(addrs)
            ));
        }
    }
    s.push_str("]}");
    s
}

/// 内核路由表（ip route）-> raw。画 JSON 仍然合法。
pub fn route_table() -> String {
    let out = cmd("ip route show 2>/dev/null || route -n 2>/dev/null");
    match out {
        Some(o) => format!("{{\"ok\":true,\"routes\":\"{}\"}}", json::jesc(&o)),
        None => "{\"ok\":false,\"msg\":\"无法读取路由表\"}".to_string(),
    }
}

/// 公网 IPv4（走外部回显服务，离线时失败）。
pub fn public_ip() -> String {
    let out = cmd("curl -s --max-time 4 https://api.ipify.org 2>/dev/null");
    match out {
        Some(ip) if !ip.trim().is_empty() => format!(
            "{{\"ok\":true,\"ip\":\"{}\"}}",
            json::jesc(ip.trim())
        ),
        _ => "{\"ok\":false,\"msg\":\"网络不可达或已离线\"}".to_string(),
    }
}

/// 系统运行时长（秒）。
pub fn uptime() -> String {
    let secs = pfile("/proc/uptime")
        .and_then(|s| s.split_whitespace().next().map(|x| x.parse::<u64>().unwrap_or(0)))
        .unwrap_or(0);
    format!(
        "{{\"ok\":true,\"seconds\":{},\"str\":\"{:02}:{:02}:{:02}\"}}",
        secs,
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// 主机名。
pub fn hostname() -> String {
    let h = pfile("/proc/sys/kernel/hostname").unwrap_or_default();
    format!("{{\"ok\":true,\"hostname\":\"{}\"}}", json::jesc(&h))
}

/// 当前在线用户（who）。
pub fn who_online() -> String {
    let out = cmd("who");
    match out {
        Some(o) if !o.is_empty() => format!("{{\"ok\":true,\"sessions\":\"{}\"}}", json::jesc(&o)),
        _ => "{\"ok\":true,\"sessions\":\"(无)\"}".to_string(),
    }
}

/// 最近登录记录（last -n 10）。
pub fn last_logins() -> String {
    let out = cmd("last -n 10 2>/dev/null");
    match out {
        Some(o) if !o.is_empty() => format!("{{\"ok\":true,\"logins\":\"{}\"}}", json::jesc(&o)),
        _ => "{\"ok\":true,\"logins\":\"(无)\"}".to_string(),
    }
}

/// 每核心 CPU 使用率（双次采样求差）。
pub fn cpu_per_core() -> String {
    let a = proc_per_core();
    std::thread::sleep(std::time::Duration::from_millis(200));
    let b = proc_per_core();
    let mut first = true;
    let mut s = String::from("{\"ok\":true,\"cores\":[");
    for (name, old) in a {
        if let Some((total, idle)) = b.get(&name) {
            if *total > old.0 {
                let tt = (*total - old.0) as f64;
                let ii = (*idle - old.1) as f64;
                let pct = ((tt - ii) / tt * 100.0).max(0.0);
                if !first {
                    s.push(',');
                }
                first = false;
                s.push_str(&format!(
                    "{{\"cpu\":\"{}\",\"usage_pct\":{:.1}}}",
                    json::jesc(&name),
                    pct
                ));
            }
        }
    }
    s.push_str("]}");
    s
}

/// 读取 /proc/stat 第 cpuN 行 -> map[cpu]:(total, idle)。
fn proc_per_core() -> std::collections::HashMap<String, (u64, u64)> {
    let mut m = std::collections::HashMap::new();
    if let Some(s) = pfile("/proc/stat") {
        for line in s.lines() {
            if !line.starts_with("cpu") {
                continue;
            }
            let mut it = line.split_whitespace();
            let name = it.next().unwrap_or("").to_string();
            let vals: Vec<u64> = it.map(|x| x.parse().unwrap_or(0)).collect();
            if vals.len() >= 4 {
                let idle = vals[3];
                let total: u64 = vals.iter().sum();
                m.insert(name, (total, idle));
            }
        }
    }
    m
}

/// 僵尸/僵停进程计数。
pub fn zombie_count() -> String {
    let n = cmd("ps -eo stat 2>/dev/null")
        .map(|o| o.lines().filter(|l| l.trim().starts_with('Z')).count())
        .unwrap_or(0);
    format!("{{\"ok\":true,\"zombies\":{}}}", n)
}

/// 查看文件末尾 N 行（tail 语义）。
pub fn file_tail(path: &str, n: usize) -> String {
    let n = if n == 0 { 50 } else { n.min(2000) };
    let (o, e) = cmd_e(&format!("tail -n {} {} 2>&1", n, shq(path)));
    if !o.is_empty() {
        format!("{{\"ok\":true,\"path\":\"{}\",\"out\":\"{}\"}}", json::jesc(path), json::jesc(&o))
    } else {
        format!("{{\"ok\":false,\"path\":\"{}\",\"msg\":\"{}\"}}", json::jesc(path), json::jesc(&e))
    }
}

/// 当前时间（epoch 秒 + UTC ISO）。
pub fn time_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // UTC 文本时间。
    let iso = iso_from_secs(secs);
    format!("{{\"ok\":true,\"epoch\":{},\"iso_gmt\":\"{}\"}}", secs, iso)
}

/// 把 epoch 秒换算成 UTC 的 ISO8601（不含时区后缀）。
fn iso_from_secs(secs: u64) -> String {
    let days0 = secs as i64 / 86400;
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // 格里历 day->date（1970-01-01 = day 0）。
    let (y, mo, d) = civil_from_days(days0);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", y, mo, d, h, m, s)
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 生成 [min,max] 区间内的随机 int。
pub fn random_int(min: i64, max: i64) -> String {
    use rand::Rng;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    let mut r = StdRng::from_entropy();
    let v = if max > min { r.gen_range(min..=max) } else { min };
    format!("{{\"ok\":true,\"value\":{}}}", v)
}

/// 读取一个环境变量。
pub fn read_env(name: &str) -> String {
    match std::env::var(name) {
        Ok(v) => format!("{{\"ok\":true,\"name\":\"{}\",\"value\":\"{}\"}}", json::jesc(name), json::jesc(&v)),
        Err(_) => format!("{{\"ok\":false,\"name\":\"{}\",\"msg\":\"未设置\"}}", json::jesc(name)),
    }
}

// ---------------------------------------------------------------------------
// 工具：shell 引号
// ---------------------------------------------------------------------------

fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}