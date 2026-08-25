//! 运维工具箱 · 第二批（`ops2`）——再补 100 个纯函数、无状态工具。
//!
//! 定位：全部「按需执行、随求即释」，只封装系统命令 / 读 /proc / 纯计算，
//! 不新增常驻状态，常驻内存维持有界（release 精简构建 ≈2MB）。
//! 命名不与第一批（ops / extra / api）重复。

use crate::json;

/// 只读命令：成功才返回 stdout。
fn cmd(c: &str) -> Option<String> {
    let out = std::process::Command::new("/bin/sh").arg("-c").arg(c).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        None
    }
}

/// 命令，失败也返回 (stdout, stderr)。
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

fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// 常见命令是否存在的单字符判断。
fn has_probe(probe: &str) -> bool {
    cmd(&format!("command -v {} >/dev/null 2>&1", probe)).is_some()
}

// ===========================================================================
// A. 网络诊断进阶
// ===========================================================================

pub fn arp_table() -> String {
    let o = cmd("ip neigh 2>/dev/null || arp -n 2>/dev/null");
    match o {
        Some(x) => format!("{{\"ok\":true,\"table\":\"{}\"}}", json::jesc(&x)),
        None => "{\"ok\":false,\"msg\":\"不可用\"}".to_string(),
    }
}
pub fn dns_mx(host: &str) -> String {
    let (out, err) = cmd_e(&format!("nslookup -type=MX {} 2>&1 || dig +short MX {} 2>&1", shq(host), shq(host)));
    if out.is_empty() {
        format!("{{\"ok\":false,\"host\":\"{}\",\"msg\":\"{}\"}}", json::jesc(host), json::jesc(&err))
    } else {
        format!("{{\"ok\":true,\"host\":\"{}\",\"mx\":\"{}\"}}", json::jesc(host), json::jesc(&out))
    }
}
pub fn dns_ns(host: &str) -> String {
    let (out, _) = cmd_e(&format!("nslookup -type=NS {} 2>&1 || dig +short NS {} 2>&1", shq(host), shq(host)));
    if out.is_empty() {
        format!("{{\"ok\":false,\"host\":\"{}\"}}", json::jesc(host))
    } else {
        format!("{{\"ok\":true,\"host\":\"{}\",\"ns\":\"{}\"}}", json::jesc(host), json::jesc(&out))
    }
}
pub fn dns_txt(host: &str) -> String {
    let (out, _) = cmd_e(&format!("dig +short TXT {} 2>&1 || nslookup -type=TXT {} 2>&1", shq(host), shq(host)));
    if out.is_empty() {
        format!("{{\"ok\":false,\"host\":\"{}\"}}", json::jesc(host))
    } else {
        format!("{{\"ok\":true,\"host\":\"{}\",\"txt\":\"{}\"}}", json::jesc(host), json::jesc(&out))
    }
}
pub fn traceroute_run(host: &str) -> String {
    let (out, err) = cmd_e(&format!("traceroute -m 8 -w 2 {} 2>&1 || tracepath {} 2>&1", shq(host), shq(host)));
    if out.is_empty() {
        format!("{{\"ok\":false,\"host\":\"{}\",\"msg\":\"{}\"}}", json::jesc(host), json::jesc(&err))
    } else {
        format!("{{\"ok\":true,\"host\":\"{}\",\"path\":\"{}\"}}", json::jesc(host), json::jesc(&out))
    }
}
pub fn tcp_state_summary() -> String {
    let o = cmd("ss -tan 2>/dev/null | awk 'NR>1{print $1}' | sort | uniq -c | sort -rn");
    match o {
        Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"states\":\"{}\"}}", json::jesc(&x)),
        _ => "{\"ok\":true,\"states\":\"\"}".to_string(),
    }
}
pub fn established_count() -> String {
    let n = cmd("ss -tan state established 2>/dev/null | tail -n +2 | wc -l")
        .and_then(|x| x.parse::<u64>().ok())
        .unwrap_or(0);
    format!("{{\"ok\":true,\"established\":{}}}", n)
}
pub fn listen_ipv6() -> String {
    let o = cmd("ss -tln6 2>/dev/null || netstat -tln6 2>/dev/null");
    match o {
        Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"ipv6_listen\":\"{}\"}}", json::jesc(&x)),
        _ => "{\"ok\":true,\"ipv6_listen\":\"(无)\"}".to_string(),
    }
}
pub fn gateway_ip() -> String {
    let o = cmd("ip route show default 2>/dev/null | awk '{print $3}'");
    match o {
        Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"gateway\":\"{}\"}}", json::jesc(&x)),
        _ => "{\"ok\":false,\"msg\":\"无默认路由\"}".to_string(),
    }
}
pub fn mac_by_ip(ip: &str) -> String {
    let o = cmd(&format!("ip neigh 2>/dev/null | grep -w {} | awk '{{print $5}}'", shq(ip)));
    match o {
        Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"ip\":\"{}\",\"mac\":\"{}\"}}", json::jesc(ip), json::jesc(&x)),
        _ => format!("{{\"ok\":false,\"ip\":\"{}\"}}", json::jesc(ip)),
    }
}

// ===========================================================================
// B. 系统纵深
// ===========================================================================

pub fn os_version_id() -> String {
    let v = pfile("/etc/os-release")
        .and_then(|s| s.lines().find(|l| l.starts_with("VERSION_ID=")).map(|l| l.trim_start_matches("VERSION_ID=").trim_matches('"').to_string()))
        .unwrap_or_default();
    format!("{{\"ok\":true,\"version_id\":\"{}\"}}", json::jesc(&v))
}
pub fn arch() -> String {
    let a = cmd("uname -m 2>/dev/null").unwrap_or_default();
    format!("{{\"ok\":true,\"arch\":\"{}\"}}", json::jesc(&a))
}
pub fn core_count() -> String {
    let n = cmd("nproc 2>/dev/null").and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"cores\":{}}}", n)
}
pub fn context_switches() -> String {
    let n = pfile("/proc/stat")
        .and_then(|s| s.lines().find(|l| l.starts_with("ctxt ")).and_then(|l| l.split_whitespace().nth(1).map(|x| x.to_string())))
        .unwrap_or_default();
    format!("{{\"ok\":true,\"ctxt\":\"{}\"}}", json::jesc(&n))
}
pub fn processes_count() -> String {
    let n = pfile("/proc/stat")
        .and_then(|s| s.lines().find(|l| l.starts_with("processes ")).and_then(|l| l.split_whitespace().nth(1).map(|x| x.to_string())))
        .unwrap_or_default();
    format!("{{\"ok\":true,\"processes\":\"{}\"}}", json::jesc(&n))
}
pub fn processes_blocked() -> String {
    let n = pfile("/proc/stat")
        .and_then(|s| s.lines().find(|l| l.starts_with("procs_blocked ")).and_then(|l| l.split_whitespace().nth(1).map(|x| x.to_string())))
        .unwrap_or_default();
    format!("{{\"ok\":true,\"blocked\":\"{}\"}}", json::jesc(&n))
}
pub fn processes_running() -> String {
    let n = pfile("/proc/stat")
        .and_then(|s| s.lines().find(|l| l.starts_with("procs_running ")).and_then(|l| l.split_whitespace().nth(1).map(|x| x.to_string())))
        .unwrap_or_default();
    format!("{{\"ok\":true,\"running\":\"{}\"}}", json::jesc(&n))
}
pub fn boot_time() -> String {
    let n = pfile("/proc/stat")
        .and_then(|s| s.lines().find(|l| l.starts_with("btime ")).and_then(|l| l.split_whitespace().nth(1).map(|x| x.to_string())))
        .unwrap_or_default();
    format!("{{\"ok\":true,\"btime\":\"{}\"}}", json::jesc(&n))
}
pub fn cache_mem() -> String {
    let kb = meminfo_value("Cached:");
    format!("{{\"ok\":true,\"cached_mb\":{:.1}}}", kb as f64 / 1024.0)
}
pub fn mem_available() -> String {
    let kb = meminfo_value("MemAvailable:");
    format!("{{\"ok\":true,\"avail_mb\":{:.1}}}", kb as f64 / 1024.0)
}

fn meminfo_value(key: &str) -> u64 {
    pfile("/proc/meminfo")
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse::<u64>().ok()))
        })
        .unwrap_or(0)
}

// ===========================================================================
// C. 文件操作
// ===========================================================================

pub fn file_stat(path: &str) -> String {
    let (out, err) = cmd_e(&format!("stat -c '%F|%s|%a|%U|%G|%y' {} 2>&1", shq(path)));
    if out.is_empty() {
        format!("{{\"ok\":false,\"path\":\"{}\",\"msg\":\"{}\"}}", json::jesc(path), json::jesc(&err))
    } else {
        format!("{{\"ok\":true,\"path\":\"{}\",\"stat\":\"{}\"}}", json::jesc(path), json::jesc(&out))
    }
}
pub fn file_copy(src: &str, dst: &str) -> (bool, String) {
    let (o, e) = cmd_e(&format!("cp -r {} {} 2>&1", shq(src), shq(dst)));
    if o.is_empty() && e.is_empty() {
        (true, format!("已复制 {} -> {}", src, dst))
    } else {
        (false, if o.is_empty() { e } else { o })
    }
}
pub fn file_delete(path: &str) -> (bool, String) {
    let (o, e) = cmd_e(&format!("rm -rf {} 2>&1", shq(path)));
    if o.is_empty() && e.is_empty() { (true, format!("已删除 {}", path)) } else { (false, if o.is_empty() { e } else { o }) }
}
pub fn file_touch(path: &str) -> (bool, String) {
    let (o, e) = cmd_e(&format!("touch {} 2>&1", shq(path)));
    if o.is_empty() && e.is_empty() { (true, format!("已 touch {}", path)) } else { (false, if o.is_empty() { e } else { o }) }
}
pub fn file_append(path: &str, content: &str) -> (bool, String) {
    match std::fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut f) => {
            use std::io::Write;
            match f.write_all(content.as_bytes()).and_then(|_| f.write_all(b"\n")) {
                Ok(_) => (true, format!("已追加到 {}", path)),
                Err(e) => (false, e.to_string()),
            }
        }
        Err(e) => (false, e.to_string()),
    }
}
pub fn file_find(dir: &str, name: &str) -> String {
    let (out, _) = cmd_e(&format!("find {} -name '{}' -type f 2>/dev/null | head -50", shq(dir), name.replace('\'', "")));
    if out.is_empty() {
        format!("{{\"ok\":true,\"dir\":\"{}\",\"found\":[]}}", json::jesc(dir))
    } else {
        let arr = format!("[{}]", out.split('\n').map(|l| format!("{}", json::jesc(l))).collect::<Vec<_>>().join(","));
        format!("{{\"ok\":true,\"dir\":\"{}\",\"found\":{}}}", json::jesc(dir), arr)
    }
}
pub fn file_md5(path: &str) -> String {
    let (out, err) = cmd_e(&format!("md5sum {} 2>&1", shq(path)));
    let sum = out.split_whitespace().next().unwrap_or("");
    format!("{{\"ok\":{},\"path\":\"{}\",\"md5\":\"{}\",\"msg\":\"{}\"}}", if sum.is_empty() { "false" } else { "true" }, json::jesc(path), sum, json::jesc(&err))
}
pub fn file_wc(path: &str) -> String {
    let (out, err) = cmd_e(&format!("wc -lc {} 2>&1", shq(path)));
    if out.is_empty() {
        format!("{{\"ok\":false,\"path\":\"{}\",\"msg\":\"{}\"}}", json::jesc(path), json::jesc(&err))
    } else {
        format!("{{\"ok\":true,\"path\":\"{}\",\"wc\":\"{}\"}}", json::jesc(path), json::jesc(&out))
    }
}
pub fn du_root(path: &str) -> String {
    let (out, err) = cmd_e(&format!("du -sh {} 2>&1", shq(path)));
    if out.is_empty() {
        format!("{{\"ok\":false,\"path\":\"{}\",\"msg\":\"{}\"}}", json::jesc(path), json::jesc(&err))
    } else {
        format!("{{\"ok\":true,\"path\":\"{}\",\"size\":\"{}\"}}", json::jesc(path), json::jesc(&out))
    }
}
pub fn ln_symlink(target: &str, link: &str) -> (bool, String) {
    let (o, e) = cmd_e(&format!("ln -s {} {} 2>&1", shq(target), shq(link)));
    if o.is_empty() && e.is_empty() { (true, format!("已建软链 {} -> {}", link, target)) } else { (false, if o.is_empty() { e } else { o }) }
}

// ===========================================================================
// D. 进程管理
// ===========================================================================

pub fn process_tree() -> String {
    let o = cmd("ps -eo pid,ppid,stat,comm --forest 2>/dev/null | head -60");
    match o { Some(x) => format!("{{\"ok\":true,\"tree\":\"{}\"}}", json::jesc(&x)), None => "{\"ok\":false}".to_string() }
}
pub fn process_threads_of(pid: u32) -> String {
    let n = cmd(&format!("ls /proc/{}/task 2>/dev/null | wc -l", pid)).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"pid\":{},\"threads\":{}}}", pid, n)
}
pub fn process_children_of(pid: u32) -> String {
    let o = cmd(&format!("pgrep -P {} 2>/dev/null | tr '\\n' ','", pid)).unwrap_or_default();
    format!("{{\"ok\":true,\"pid\":{},\"children\":\"{}\"}}", pid, json::jesc(&o))
}
pub fn process_cwd(pid: u32) -> String {
    let c = cmd(&format!("readlink /proc/{}/cwd 2>/dev/null", pid)).unwrap_or_default();
    format!("{{\"ok\":true,\"pid\":{},\"cwd\":\"{}\"}}", pid, json::jesc(&c))
}
pub fn process_cmdline(pid: u32) -> String {
    let c = pfile(&format!("/proc/{}/cmdline", pid))
        .map(|s| s.replace('\0', " ").trim().to_string())
        .unwrap_or_default();
    format!("{{\"ok\":true,\"pid\":{},\"cmdline\":\"{}\"}}", pid, json::jesc(&c))
}
pub fn process_top_cpu(n: u32) -> String {
    let n = if n == 0 { 10 } else { n.min(50) };
    let o = cmd(&format!("ps -eo pid,pcpu,comm --sort=-pcpu 2>/dev/null | head -n {}", n + 1));
    match o { Some(x) => format!("{{\"ok\":true,\"top_cpu\":\"{}\"}}", json::jesc(&x)), None => "{\"ok\":false}".to_string() }
}
pub fn process_top_mem(n: u32) -> String {
    let n = if n == 0 { 10 } else { n.min(50) };
    let o = cmd(&format!("ps -eo pid,pmem,rss,comm --sort=-rss 2>/dev/null | head -n {}", n + 1));
    match o { Some(x) => format!("{{\"ok\":true,\"top_mem\":\"{}\"}}", json::jesc(&x)), None => "{\"ok\":false}".to_string() }
}
pub fn process_state_count() -> String {
    let o = cmd("ps -eo stat 2>/dev/null | awk 'NR>1{s=substr($1,1,1);c[s]++}END{for(k in c)printf \"%s=%d \",k,c[k]}'");
    match o { Some(x) => format!("{{\"ok\":true,\"states\":\"{}\"}}", json::jesc(&x)), None => "{\"ok\":false}".to_string() }
}
pub fn kill_process_by_name(name: &str) -> (bool, String) {
    let (o, e) = cmd_e(&format!("pkill -f {} 2>&1", shq(name)));
    if o.is_empty() && e.is_empty() { (true, format!("已结束包含 {} 的进程", name)) } else { (false, if o.is_empty() { e } else { o }) }
}
pub fn nice_set(pid: u32, nice: i32) -> (bool, String) {
    let (o, e) = cmd_e(&format!("renice {} {} 2>&1", nice, pid));
    if o.is_empty() && e.is_empty() { (true, format!("pid {} 优先级设为 {}", pid, nice)) } else { (false, if o.is_empty() { e } else { o }) }
}

// ===========================================================================
// E. 软件 / 包
// ===========================================================================

pub fn apt_search(keyword: &str) -> String {
    let (out, _) = cmd_e(&format!("apt-cache search {} 2>/dev/null | head -30", shq(keyword)));
    if out.is_empty() { format!("{{\"ok\":true,\"q\":\"{}\",\"hits\":[]}}", json::jesc(keyword)) }
    else { format!("{{\"ok\":true,\"q\":\"{}\",\"hits\":\"{}\"}}", json::jesc(keyword), json::jesc(&out)) }
}
pub fn apt_pkg_info(pkg: &str) -> String {
    let (out, err) = cmd_e(&format!("apt-cache show {} 2>&1 || dpkg -s {} 2>&1", shq(pkg), shq(pkg)));
    if out.is_empty() { format!("{{\"ok\":false,\"pkg\":\"{}\",\"msg\":\"{}\"}}", json::jesc(pkg), json::jesc(&err)) }
    else { format!("{{\"ok\":true,\"pkg\":\"{}\",\"info\":\"{}\"}}", json::jesc(pkg), json::jesc(&out)) }
}
pub fn apt_depends(pkg: &str) -> String {
    let (out, err) = cmd_e(&format!("apt-cache depends {} 2>&1", shq(pkg)));
    if out.is_empty() { format!("{{\"ok\":false,\"pkg\":\"{}\",\"msg\":\"{}\"}}", json::jesc(pkg), json::jesc(&err)) }
    else { format!("{{\"ok\":true,\"pkg\":\"{}\",\"depends\":\"{}\"}}", json::jesc(pkg), json::jesc(&out)) }
}
pub fn dpkg_count() -> String {
    let n = cmd("dpkg -l 2>/dev/null | tail -n +6 | wc -l").and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"installed\":{}}}", n)
}
pub fn pip_version() -> String {
    let o = cmd("pip3 --version 2>/dev/null || pip --version 2>/dev/null").unwrap_or_default();
    format!("{{\"ok\":true,\"pip\":\"{}\"}}", json::jesc(&o))
}
pub fn nginx_version() -> String {
    let o = cmd("nginx -v 2>&1").unwrap_or_default();
    format!("{{\"ok\":true,\"nginx\":\"{}\"}}", json::jesc(&o))
}
pub fn redis_version() -> String {
    let o = cmd("redis-server --version 2>/dev/null || redis-cli --version 2>/dev/null").unwrap_or_default();
    format!("{{\"ok\":true,\"redis\":\"{}\"}}", json::jesc(&o))
}
pub fn docker_version() -> String {
    let o = cmd("docker --version 2>/dev/null").unwrap_or_default();
    format!("{{\"ok\":true,\"docker\":\"{}\"}}", json::jesc(&o))
}
pub fn git_version() -> String {
    let o = cmd("git --version 2>/dev/null").unwrap_or_default();
    format!("{{\"ok\":true,\"git\":\"{}\"}}", json::jesc(&o))
}
pub fn curl_version() -> String {
    let o = cmd("curl --version 2>/dev/null | head -1").unwrap_or_default();
    format!("{{\"ok\":true,\"curl\":\"{}\"}}", json::jesc(&o))
}

// ===========================================================================
// F. 服务 / 定时
// ===========================================================================

pub fn systemd_failed() -> String {
    let o = cmd("systemctl --failed --no-legend 2>/dev/null || systemctl list-units --state=failed --no-legend 2>/dev/null");
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"failed\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"failed\":\"(无)\"}".to_string() }
}
pub fn systemd_enabled() -> String {
    let o = cmd("systemctl list-unit-files --state=enabled --no-legend 2>/dev/null | head -40");
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"enabled\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"enabled\":\"(无)\"}".to_string() }
}
pub fn systemd_timers() -> String {
    let o = cmd("systemctl list-timers --no-legend 2>/dev/null | head -20");
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"timers\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"timers\":\"(无)\"}".to_string() }
}
pub fn port_owner(port: u32) -> String {
    let o = cmd(&format!("ss -tlnp 2>/dev/null | grep ':{}\\s' || ss -tlnp 2>/dev/null | grep ':{}\\.'", port, port));
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"port\":{},\"owner\":\"{}\"}}", port, json::jesc(&x)), _ => format!("{{\"ok\":false,\"port\":{}}}", port) }
}
pub fn cron_full() -> String {
    let o = cmd("crontab -l 2>/dev/null | grep -v '^#' | grep -v '^$'");
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"crontab\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"crontab\":\"(空)\"}".to_string() }
}
pub fn at_jobs() -> String {
    let o = cmd("atq 2>/dev/null");
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"at\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"at\":\"(无)\"}".to_string() }
}
pub fn wanted_units() -> String {
    let o = cmd("ls /etc/systemd/system/multi-user.target.wants/ 2>/dev/null | head -40");
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"wanted\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"wanted\":\"(无)\"}".to_string() }
}
pub fn login_sessions() -> String {
    let o = cmd("loginctl list-sessions --no-legend 2>/dev/null | head -20");
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"sessions\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"sessions\":\"(无)\"}".to_string() }
}
pub fn journal_size() -> String {
    let o = cmd("journalctl --disk-usage 2>/dev/null || du -sh /var/log/journal 2>/dev/null").unwrap_or_default();
    format!("{{\"ok\":true,\"journal\":\"{}\"}}", json::jesc(&o))
}
pub fn tmp_count() -> String {
    let n = cmd("find /tmp -maxdepth 1 -type f 2>/dev/null | wc -l").and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"tmp_files\":{}}}", n)
}

// ===========================================================================
// G. 安全加固
// ===========================================================================

pub fn uid0_users() -> String {
    let o = cmd("awk -F: '$3==0{print $1}' /etc/passwd 2>/dev/null | tr '\\n' ','");
    format!("{{\"ok\":true,\"uid0\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn sudoers_users() -> String {
    let o = cmd("grep -E '^sudo|^wheel' /etc/group 2>/dev/null | awk -F: '{print $4}' | tr '\\n' ','");
    format!("{{\"ok\":true,\"sudoers\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn ssh_keys_present() -> String {
    let o = cmd("grep -l 'ssh-' /root/.ssh/authorized_keys /home/*/.ssh/authorized_keys 2>/dev/null | tr '\\n' ','");
    format!("{{\"ok\":true,\"auth_keys\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn ssh_param(param: &str) -> String {
    let out = cmd(&format!("sshd -T 2>/dev/null | grep -i '^{}\\s' | head -1 || grep -E '^{}' /etc/ssh/sshd_config 2>/dev/null | grep -v '^#' | head -1", shq(param), shq(param)));
    format!("{{\"ok\":safe,\"param\":\"{}\",\"value\":\"{}\"}}", json::jesc(param), json::jesc(&out.unwrap_or_default())).replace("\"ok\":safe", "\"ok\":true")
}
pub fn open_ports_summary() -> String {
    let o = cmd("ss -tln 2>/dev/null | tail -n +2 | awk '{print $4}' | sed 's/.*://' | sort -n | uniq -c | sort -rn");
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"ports\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"ports\":\"\"}".to_string() }
}
pub fn pending_upgrades() -> String {
    let n = cmd("apt list --upgradable 2>/dev/null | tail -n +2 | wc -l").and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"pending\":{}}}", n)
}
pub fn failed_auths(n: u32) -> String {
    let n = if n == 0 { 20 } else { n.min(200) };
    let o = cmd(&format!("grep -i 'Failed password' /var/log/auth.log* 2>/dev/null | tail -n {} | sort | uniq -c | sort -rn | head -20", n));
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"failed\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"failed\":\"(无)\"}".to_string() }
}
pub fn listening_uid_owners() -> String {
    let o = cmd("ss -tlnp 2>/dev/null | tail -n +2 | awk '{print $4, $6}' | head -30");
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"owners\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"owners\":\"\"}".to_string() }
}
pub fn mounts_with_exec() -> String {
    let o = cmd("findmnt -o TARGET,OPTIONS -n 2>/dev/null | grep exec | grep -v 'noexec' | head -20");
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"exec_mounts\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"exec_mounts\":\"(无)\"}".to_string() }
}
pub fn sensitive_perms() -> String {
    let o = cmd("ls -l /etc/shadow /etc/passwd /etc/sudoers 2>/dev/null");
    match o { Some(x) => format!("{{\"ok\":true,\"perms\":\"{}\"}}", json::jesc(&x)), None => "{\"ok\":false}".to_string() }
}

// ===========================================================================
// H. 数据 / 编码（纯实现，无外部依赖）
// ===========================================================================

pub fn md5_digest(text: &str) -> String {
    let out = cmd(&format!("printf '%s' {} | md5sum 2>/dev/null || printf '%s' {} | md5 ", shq(text), shq(text)));
    let sum = out.map(|o| o.split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default();
    format!("{{\"ok\":true,\"md5\":\"{}\",\"len\":{}}}", sum, text.len())
}
pub fn sha1_digest(text: &str) -> String {
    use sha1::Digest;
    let hex = sha1::Sha1::digest(text.as_bytes()).iter().map(|b| format!("{:02x}", b)).collect::<String>();
    format!("{{\"ok\":true,\"sha1\":\"{}\"}}", hex)
}
pub fn cksum_text(text: &str) -> String {
    let o = cmd(&format!("printf '%s' {} | cksum 2>&1", shq(text)));
    let out = o.unwrap_or_default();
    let parts: Vec<&str> = out.split_whitespace().collect();
    let (crc, bytes) = if parts.len() >= 2 { (parts[0], parts[1]) } else { ("", "") };
    format!("{{\"ok\":true,\"crc32\":\"{}\",\"bytes\":\"{}\"}}", crc, bytes)
}
pub fn url_encode(text: &str) -> String {
    let mut o = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { o.push(b as char); }
        else { o.push_str(&format!("%{:02X}", b)); }
    }
    format!("{{\"ok\":true,\"encoded\":\"{}\"}}", json::jesc(&o))
}
pub fn url_decode(text: &str) -> String {
    let b = text.as_bytes();
    let mut o: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hi = hexv(b[i + 1]); let lo = hexv(b[i + 2]);
            if hi < 16 && lo < 16 { o.push(hi * 16 + lo); i += 3; continue; }
        } else if b[i] == b'+' { o.push(b' '); i += 1; continue; }
        o.push(b[i]); i += 1;
    }
    format!("{{\"ok\":true,\"decoded\":\"{}\"}}", json::jesc(&String::from_utf8_lossy(&o)))
}
fn hexv(c: u8) -> u8 {
    match c { b'0'..=b'9' => c - b'0', b'a'..=b'f' => c - b'a' + 10, b'A'..=b'F' => c - b'A' + 10, _ => 16 }
}
pub fn hex_encode(text: &str) -> String {
    let hex = text.as_bytes().iter().map(|b| format!("{:02x}", b)).collect::<String>();
    format!("{{\"ok\":true,\"hex\":\"{}\"}}", json::jesc(&hex))
}
pub fn hex_decode(text: &str) -> String {
    let t = text.as_bytes();
    let mut o = Vec::new();
    let mut i = 0;
    while i + 1 < t.len() {
        let hi = hexv(t[i]); let lo = hexv(t[i + 1]);
        if hi < 16 && lo < 16 { o.push(hi * 16 + lo); }
        i += 2;
    }
    format!("{{\"ok\":true,\"dec\":\"{}\"}}", json::jesc(&String::from_utf8_lossy(&o)))
}
pub fn base32_encode(text: &str) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let b = text.as_bytes();
    let mut out = String::new();
    let mut bits = 0u32; let mut n = 0u32;
    for &x in b { bits = (bits << 8) | x as u32; n += 8; while n >= 5 { n -= 5; out.push(A[((bits >> n) & 31) as usize] as char); } }
    if n > 0 { out.push(A[((bits << (5 - n)) & 31) as usize] as char); }
    while out.len() % 8 != 0 { out.push('='); }
    format!("{{\"ok\":true,\"base32\":\"{}\"}}", json::jesc(&out))
}
pub fn upper_case(text: &str) -> String {
    format!("{{\"ok\":true,\"out\":\"{}\"}}", json::jesc(&text.to_uppercase()))
}
pub fn lower_case(text: &str) -> String {
    format!("{{\"ok\":true,\"out\":\"{}\"}}", json::jesc(&text.to_lowercase()))
}

// ===========================================================================
// I. 文本 / 处理
// ===========================================================================

pub fn wc_lines(path: &str) -> String {
    let n = cmd(&format!("wc -l < {} 2>/dev/null", shq(path))).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"path\":\"{}\",\"lines\":{}}}", json::jesc(path), n)
}
pub fn wc_words(path: &str) -> String {
    let n = cmd(&format!("wc -w < {} 2>/dev/null", shq(path))).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"path\":\"{}\",\"words\":{}}}", json::jesc(path), n)
}
pub fn grep_count(path: &str, pattern: &str) -> String {
    let n = cmd(&format!("grep -c '{}' {} 2>/dev/null", pattern.replace('\'', ""), shq(path))).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"path\":\"{}\",\"pattern\":\"{}\",\"count\":{}}}", json::jesc(path), json::jesc(pattern), n)
}
pub fn grep_lines(path: &str, pattern: &str) -> String {
    let o = cmd(&format!("grep -n '{}' {} 2>/dev/null | head -50", pattern.replace('\'', ""), shq(path)));
    format!("{{\"ok\":true,\"path\":\"{}\",\"lines\":\"{}\"}}", json::jesc(path), json::jesc(&o.unwrap_or_default()))
}
pub fn sort_numeric(text: &str) -> String {
    let mut v: Vec<i64> = text.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    v.sort_unstable();
    let out = v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
    format!("{{\"ok\":true,\"sorted\":[{}]}}", out)
}
pub fn unique_lines(text: &str) -> String {
    let mut v: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    v.sort_unstable();
    v.dedup();
    format!("{{\"ok\":true,\"unique\":\"{}\"}}", json::jesc(&v.join("\n")))
}
pub fn cut_field(text: &str, delim: &str, field: u32) -> String {
    let f = field.max(1) as usize;
    let out: Vec<&str> = text.split(&*delim).nth(f - 1).map(|s| s.trim()).filter(|s| !s.is_empty()).into_iter().collect::<Vec<_>>();
    format!("{{\"ok\":true,\"value\":\"{}\"}}", json::jesc(out.first().copied().unwrap_or("")))
}
pub fn tr_replace(text: &str, from: &str, to: &str) -> String {
    let out = text.replace(from, to);
    format!("{{\"ok\":true,\"out\":\"{}\"}}", json::jesc(&out))
}
pub fn append_line_once(path: &str, line: &str) -> (bool, String) {
    let found = cmd(&format!("grep -Fxq {} {} 2>/dev/null && echo yes", shq(line), shq(path))).map(|x| x.contains("yes")).unwrap_or(false);
    if found { (false, "行已存在，未追加".to_string()) }
    else { file_append(path, line) }
}
pub fn csv_fields(text: &str, sep: &str) -> String {
    let n = text.split(&*sep).count();
    format!("{{\"ok\":true,\"fields\":{}}}", n)
}

// ===========================================================================
// J. 杂项 / 时间 / 校验
// ===========================================================================

pub fn epoch_to_time(epoch: i64) -> String {
    let secs = if epoch < 0 { 0u64 } else { epoch as u64 };
    let days = secs as i64 / 86400;
    let rem = secs % 86400;
    let h = rem / 3600; let m = (rem % 3600) / 60; let s = rem % 60;
    let (y, mo, d) = civil(days);
    format!("{{\"ok\":true,\"iso_gmt\":\"{:04}-{:02}-{:02}T{:02}:{:02}:{:02}\"}}", y, mo, d, h, m, s)
}
fn civil(z: i64) -> (i64, i64, i64) {
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
pub fn timezone_offset() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // 本地 tz 由系统给出，单调偏移由 TZ 配置。
    let off = *crate::config::tz();
    format!("{{\"ok\":true,\"epoch\":{},\"tz_offset_s\":{}}}", now, off)
}
pub fn random_token(len: u32) -> String {
    use rand::Rng;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    let len = if len == 0 { 32 } else { len.min(256) } as usize;
    let mut r = StdRng::from_entropy();
    let hex: String = (0..len).map(|_| format!("{:02x}", r.gen::<u8>())).collect();
    format!("{{\"ok\":true,\"token\":\"{}\"}}", hex)
}
pub fn rand_bool() -> String {
    use rand::Rng;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    let v = StdRng::from_entropy().gen::<bool>();
    format!("{{\"ok\":true,\"value\":{}}}", v)
}
pub fn valid_ip(ip: &str) -> String {
    let ok = valid_ipv4(ip);
    format!("{{\"ok\":true,\"ip\":\"{}\",\"valid\":{}}}", json::jesc(ip), ok)
}
fn valid_ipv4(s: &str) -> bool {
    let p: Vec<&str> = s.split('.').collect();
    p.len() == 4 && p.iter().all(|o| o.parse::<u8>().is_ok() && !o.is_empty())
}
pub fn valid_domain(host: &str) -> String {
    let ok = !host.is_empty() && host.len() <= 253
        && host.split('.').all(|l| !l.is_empty() && l.len() <= 63 && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
    format!("{{\"ok\":true,\"host\":\"{}\",\"valid\":{}}}", json::jesc(host), ok)
}
pub fn default_route_via() -> String {
    let o = cmd("ip route show default 2>/dev/null | awk '{print $3}' | head -1").unwrap_or_default();
    format!("{{\"ok\":true,\"via\":\"{}\"}}", json::jesc(&o))
}
pub fn dns_servers() -> String {
    let o = cmd("awk '/^nameserver/{print $2}' /etc/resolv.conf 2>/dev/null | tr '\\n' ','").unwrap_or_default();
    format!("{{\"ok\":true,\"nameservers\":\"{}\"}}", json::jesc(&o))
}
pub fn swap_usage() -> String {
    let total = meminfo_value("SwapTotal:");
    let free = meminfo_value("SwapFree:");
    let used = total.saturating_sub(free);
    format!("{{\"ok\":true,\"total_mb\":{:.1},\"used_mb\":{:.1},\"free_mb\":{:.1}}}", total as f64 / 1024.0, used as f64 / 1024.0, free as f64 / 1024.0)
}
pub fn disk_io_simple() -> String {
    let o = cmd("iostat -x 1 1 2>/dev/null | tail -20 || cat /proc/diskstats 2>/dev/null | head -30");
    format!("{{\"ok\":true,\"io\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}