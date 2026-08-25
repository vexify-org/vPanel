//! 运维工具箱 · 第三批（`ops3`）——再补 100 个纯函数、无状态工具。
//!
//! 定位：与 ops / ops2 一致，全部「按需执行、随求即释」，只封装系统命令 / 读
//! /proc / 纯计算，不新增常驻状态，常驻内存维持有界。命名不与前两批重复。

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

fn num(path: &str, key: &str) -> u64 {
    pfile(path)
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse::<u64>().ok()))
        })
        .unwrap_or(0)
}

// ===========================================================================
// K. 网络 / 流量 / 连接进阶
// ===========================================================================

pub fn iface_list() -> String {
    let o = cmd("ip -o link show 2>/dev/null | awk -F':' '{print $2}' | tr -d ' ' | tr '\\n' ',' || ls /sys/class/net | tr '\\n' ','");
    format!("{{\"ok\":true,\"ifaces\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn iface_speed(iface: &str) -> String {
    let o = cmd(&format!("cat /sys/class/net/{}/speed 2>/dev/null", shq(iface)));
    format!("{{\"ok\":true,\"iface\":\"{}\",\"speed_mbps\":\"{}\"}}", json::jesc(iface), json::jesc(&o.unwrap_or_default()))
}
pub fn iface_duplex(iface: &str) -> String {
    let o = cmd(&format!("cat /sys/class/net/{}/duplex 2>/dev/null", shq(iface)));
    format!("{{\"ok\":true,\"iface\":\"{}\",\"duplex\":\"{}\"}}", json::jesc(iface), json::jesc(&o.unwrap_or_default()))
}
pub fn iface_mac(iface: &str) -> String {
    let o = cmd(&format!("cat /sys/class/net/{}/address 2>/dev/null", shq(iface)));
    format!("{{\"ok\":true,\"iface\":\"{}\",\"mac\":\"{}\"}}", json::jesc(iface), json::jesc(&o.unwrap_or_default()))
}
pub fn iface_up(iface: &str) -> String {
    let o = cmd(&format!("ip link show {} 2>/dev/null | grep -q 'UP' && echo up || echo down", shq(iface)));
    format!("{{\"ok\":true,\"iface\":\"{}\",\"state\":\"{}\"}}", json::jesc(iface), json::jesc(&o.unwrap_or_default()))
}
pub fn traffic_since_boot() -> String {
    let o = cmd("awk '{rx+=$2;tx+=$10} END{printf \"rx:%d tx:%d rxmb:%.1f txmb:%.1f\",rx,tx,rx/1024/1024,tx/1024/1024}' /proc/net/dev");
    format!("{{\"ok\":true,\"traffic\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn udp_listen() -> String {
    let o = cmd("ss -uln 2>/dev/null | tail -n +2 | head -30");
    match o { Some(x) if !x.is_empty() => format!("{{\"ok\":true,\"udp\":\"{}\"}}", json::jesc(&x)), _ => "{\"ok\":true,\"udp\":\"(无)\"}".to_string() }
}
pub fn unix_sockets() -> String {
    let o = cmd("ss -xl 2>/dev/null | tail -n +2 | head -30");
    format!("{{\"ok\":true,\"unix\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn ip_v6_addr() -> String {
    let o = cmd("ip -6 addr show 2>/dev/null | grep inet6 | awk '{print $2}' | head -20 | tr '\\n' ','");
    format!("{{\"ok\":true,\"ipv6\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn ping_loss(host: &str) -> String {
    let (o, _) = cmd_e(&format!("ping -q -c 5 -W 2 {} 2>&1 | tail -2", shq(host)));
    format!("{{\"ok\":true,\"host\":\"{}\",\"summary\":\"{}\"}}", json::jesc(host), json::jesc(&o))
}

// ===========================================================================
// L. 系统 / 内核 / 细节
// ===========================================================================

pub fn kernel_release() -> String {
    let o = pfile("/proc/sys/kernel/osrelease").unwrap_or_default();
    format!("{{\"ok\":true,\"release\":\"{}\"}}", json::jesc(&o))
}
pub fn kernel_version() -> String {
    let o = pfile("/proc/sys/kernel/version").unwrap_or_default();
    format!("{{\"ok\":true,\"version\":\"{}\"}}", json::jesc(&o))
}
pub fn hostname_full() -> String {
    let o = pfile("/proc/sys/kernel/hostname").unwrap_or_default();
    format!("{{\"ok\":true,\"hostname\":\"{}\"}}", json::jesc(&o))
}
pub fn kernel_config(param: &str) -> String {
    let o = pfile(&format!("/proc/sys/kernel/{}", param.replace('/', "_"))).unwrap_or_default();
    format!("{{\"ok\":true,\"param\":\"kernel.{}\",\"value\":\"{}\"}}", json::jesc(param), json::jesc(&o))
}
pub fn vm_params() -> String {
    let o = cmd("sysctl vm.swappiness vm.overcommit_memory vm.dirty_ratio vm.vfs_cache_pressure 2>/dev/null");
    format!("{{\"ok\":true,\"vm\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn fs_params() -> String {
    let o = num("/proc/sys/fs/file-nr", "");
    let _ = o;
    let o = cmd("awk '{print \"nr_open:\"$1}' /proc/sys/fs/nr_open 2>/dev/null; cat /proc/sys/fs/file-nr 2>/dev/null");
    format!("{{\"ok\":true,\"fs\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn net_params() -> String {
    let o = cmd("sysctl net.ipv4.ip_forward net.ipv4.tcp_syncookies net.ipv4.tcp_tw_reuse 2>/dev/null");
    format!("{{\"ok\":true,\"net\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn entropy_avail() -> String {
    let o = pfile("/proc/sys/kernel/random/entropy_avail").unwrap_or_default();
    format!("{{\"ok\":true,\"entropy\":\"{}\"}}", json::jesc(&o))
}
pub fn allowed_ports() -> String {
    let o = pfile("/proc/sys/net/ipv4/ip_local_port_range").unwrap_or_default();
    format!("{{\"ok\":true,\"range\":\"{}\"}}", json::jesc(&o))
}
pub fn mem_zones() -> String {
    let o = pfile("/proc/zoneinfo").map(|s| s.split(' ').take(4).count().to_string()).unwrap_or_default();
    format!("{{\"ok\":true,\"zones_hint\":\"{}\"}}", json::jesc(&o))
}

// ===========================================================================
// M. 磁盘 / 挂载 / 存储
// ===========================================================================

pub fn mount_list() -> String {
    let o = cmd("mount | grep -v ' cgroup\\| proc\\| sysfs\\| devtmpfs\\| securityfs\\| tmpfs\\|overlay\\|cgroup2\\|mqueue\\|debugfs\\|tracefs\\|configfs\\|fusectl\\|pstore' | sort | head -40");
    format!("{{\"ok\":true,\"mounts\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn mount_by_point(point: &str) -> String {
    let o = cmd(&format!("findmnt {} 2>/dev/null || mount | grep ' on {} ' ", shq(point), shq(point)));
    format!("{{\"ok\":true,\"point\":\"{}\",\"mount\":\"{}\"}}", json::jesc(point), json::jesc(&o.unwrap_or_default()))
}
pub fn disk_uuid() -> String {
    let o = cmd("blkid 2>/dev/null || lsblk -o NAME,UUID 2>/dev/null | head -20");
    format!("{{\"ok\":true,\"uuid\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn disk_model() -> String {
    let o = cmd("lsblk -o NAME,MODEL,SIZE,TYPE -d -n 2>/dev/null | head -20");
    format!("{{\"ok\":true,\"disks\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn disk_readonly() -> String {
    let o = cmd("grep ' ro\\b' /proc/mounts 2>/dev/null | awk '{print $2}' | tr '\\n' ','");
    format!("{{\"ok\":true,\"ro_mounts\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn inode_usage(path: &str) -> String {
    let o = cmd(&format!("df -i {} 2>/dev/null", shq(path)));
    format!("{{\"ok\":true,\"path\":\"{}\",\"inodes\":\"{}\"}}", json::jesc(path), json::jesc(&o.unwrap_or_default()))
}
pub fn fs_type(path: &str) -> String {
    let o = cmd(&format!("stat -f -c '%T' {} 2>/dev/null", shq(path)));
    format!("{{\"ok\":true,\"path\":\"{}\",\"fstype\":\"{}\"}}", json::jesc(path), json::jesc(&o.unwrap_or_default()))
}
pub fn block_devices() -> String {
    let o = cmd("lsblk -o NAME,TYPE,SIZE,MOUNTPOINT -n 2>/dev/null | head -30");
    format!("{{\"ok\":true,\"blockdevs\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn sector_size() -> String {
    let o = cmd("blockdev --getss /dev/sda 2>/dev/null || echo 512");
    format!("{{\"ok\":true,\"sector_bytes\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn swap_devices() -> String {
    let o = cmd("swapon --show 2>/dev/null || cat /proc/swaps 2>/dev/null | head -20");
    format!("{{\"ok\":true,\"swap\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}

// ===========================================================================
// N. 用户 / 权限 / 身份
// ===========================================================================

pub fn user_home(user: &str) -> String {
    let o = cmd(&format!("getent passwd {} 2>/dev/null | cut -d: -f6", shq(user)));
    format!("{{\"ok\":true,\"user\":\"{}\",\"home\":\"{}\"}}", json::jesc(user), json::jesc(&o.unwrap_or_default()))
}
pub fn user_shell(user: &str) -> String {
    let o = cmd(&format!("getent passwd {} 2>/dev/null | cut -d: -f7", shq(user)));
    format!("{{\"ok\":true,\"user\":\"{}\",\"shell\":\"{}\"}}", json::jesc(user), json::jesc(&o.unwrap_or_default()))
}
pub fn user_groups(user: &str) -> String {
    let o = cmd(&format!("id -nG {} 2>/dev/null | tr ' ' ','", shq(user)));
    format!("{{\"ok\":true,\"user\":\"{}\",\"groups\":\"{}\"}}", json::jesc(user), json::jesc(&o.unwrap_or_default()))
}
pub fn group_members(group: &str) -> String {
    let o = cmd(&format!("getent group {} 2>/dev/null | cut -d: -f4", shq(group)));
    format!("{{\"ok\":true,\"group\":\"{}\",\"members\":\"{}\"}}", json::jesc(group), json::jesc(&o.unwrap_or_default()))
}
pub fn user_last_login(user: &str) -> String {
    let o = cmd(&format!("last -n1 -w {} 2>/dev/null", shq(user)));
    format!("{{\"ok\":true,\"user\":\"{}\",\"last\":\"{}\"}}", json::jesc(user), json::jesc(&o.unwrap_or_default()))
}
pub fn lock_users() -> String {
    let o = cmd("awk -F: '($9==\"*\\\" || $2==\"!\"){print $1\"\\n\"}' /etc/shadow 2>/dev/null | xargs -r echo | tr ' ' ',' ");
    format!("{{\"ok\":true,\"locked\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn logins_total() -> String {
    let n = cmd("last -wx 2>/dev/null | grep -c boot || last 2>/dev/null | wc -l").and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"logins\":{}}}", n)
}
pub fn nologin_users() -> String {
    let o = cmd("awk -F: '/nologin|false/{print $1}' /etc/passwd 2>/dev/null | tr '\\n' ','");
    format!("{{\"ok\":true,\"nologin\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn file_owner(path: &str) -> String {
    let o = cmd(&format!("stat -c '%U:%G' {} 2>/dev/null", shq(path)));
    format!("{{\"ok\":true,\"path\":\"{}\",\"owner\":\"{}\"}}", json::jesc(path), json::jesc(&o.unwrap_or_default()))
}
pub fn effective_uid() -> String {
    let o = cmd("id 2>/dev/null");
    format!("{{\"ok\":true,\"id\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}

// ===========================================================================
// O. 安全 / 加固 / 审计
// ===========================================================================

pub fn selinux_status() -> String {
    let o = cmd("getenforce 2>/dev/null || echo 'disabled'");
    format!("{{\"ok\":true,\"selinux\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn apparmor_status() -> String {
    let o = cmd("aa-status 2>/dev/null | head -5 || cat /sys/kernel/security/apparmor/profiles 2>/dev/null | head -5");
    format!("{{\"ok\":true,\"apparmor\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn world_writable() -> String {
    let o = cmd("find / -xdev -type f -perm -0002 2>/dev/null | head -20 | tr '\\n' '\\n'");
    format!("{{\"ok\":true,\"files\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn suid_bins() -> String {
    let o = cmd("find / -xdev -type f -perm -4000 2>/dev/null | head -30 | tr '\\n' ','");
    format!("{{\"ok\":true,\"suid\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn socket_perms() -> String {
    let o = cmd("ls -l /var/run/mysqld/mysqld.sock /run/redis/redis-server.sock /var/run/php-fpm*.sock 2>/dev/null");
    format!("{{\"ok\":true,\"sockets\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn ip_forward() -> String {
    let o = pfile("/proc/sys/net/ipv4/ip_forward").unwrap_or_default();
    format!("{{\"ok\":true,\"ip_forward\":\"{}\"}}", json::jesc(&o))
}
pub fn firewall_active() -> String {
    let o = cmd("systemctl is-active ufw firewalld nftables 2>/dev/null || echo 'inactive'");
    format!("{{\"ok\":true,\"active\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn listen_low_ports() -> String {
    let o = cmd("ss -tln 2>/dev/null | tail -n +2 | awk '{split($4,a,\":\"); if(a[2]<1024) print a[2]}' | sort -nu | tr '\\n' ','");
    format!("{{\"ok\":true,\"low_ports\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn umask_current() -> String {
    let o = cmd("umask 2>/dev/null");
    format!("{{\"ok\":true,\"umask\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn core_pattern() -> String {
    let o = pfile("/proc/sys/kernel/core_pattern").unwrap_or_default();
    format!("{{\"ok\":true,\"core_pattern\":\"{}\"}}", json::jesc(&o))
}

// ===========================================================================
// P. 文本 / 变换 / 度量
// ===========================================================================

pub fn char_count(text: &str) -> String {
    format!("{{\"ok\":true,\"chars\":{},\"bytes\":{}}}", text.chars().count(), text.len())
}
pub fn str_reverse(text: &str) -> String {
    let r: String = text.chars().rev().collect();
    format!("{{\"ok\":true,\"out\":\"{}\"}}", json::jesc(&r))
}
pub fn word_count(text: &str) -> String {
    let n = text.split_whitespace().count();
    format!("{{\"ok\":true,\"words\":{}}}", n)
}
pub fn is_empty(text: &str) -> String {
    format!("{{\"ok\":true,\"empty\":{}}}", text.trim().is_empty())
}
pub fn has_digits(text: &str) -> String {
    format!("{{\"ok\":true,\"has_digits\":{}}}", text.chars().any(|c| c.is_ascii_digit()))
}
pub fn has_upper(text: &str) -> String {
    format!("{{\"ok\":true,\"has_upper\":{}}}", text.chars().any(|c| c.is_ascii_uppercase()))
}
pub fn dashed_line() -> String {
    format!("{{\"ok\":true,\"line\":\"{}\"}}", "-".repeat(60))
}
pub fn repeat_str(text: &str, n: u32) -> String {
    let n = n.min(1000) as usize;
    format!("{{\"ok\":true,\"out\":\"{}\"}}", json::jesc(&text.repeat(n)))
}
pub fn title_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = true;
    for c in text.chars() {
        if c.is_whitespace() { prev_space = true; out.push(c); }
        else if prev_space { prev_space = false; out.extend(c.to_uppercase()); }
        else { out.push(c); }
    }
    format!("{{\"ok\":true,\"out\":\"{}\"}}", json::jesc(&out))
}
pub fn swap_case(text: &str) -> String {
    let out = text.chars().map(|c| if c.is_ascii_lowercase() { c.to_ascii_uppercase() } else if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c }).collect::<String>();
    format!("{{\"ok\":true,\"out\":\"{}\"}}", json::jesc(&out))
}

// ===========================================================================
// Q. 数学 / 统计 / 计算
// ===========================================================================

pub fn sum_list(text: &str) -> String {
    let v: Vec<i64> = text.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let s: i64 = v.iter().sum();
    format!("{{\"ok\":true,\"n\":{},\"sum\":{}}}", v.len(), s)
}
pub fn avg_list(text: &str) -> String {
    let v: Vec<f64> = text.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let a = if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
    format!("{{\"ok\":true,\"n\":{},\"avg\":{:.4}}}", v.len(), a)
}
pub fn min_max(text: &str) -> String {
    let v: Vec<i64> = text.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let (mn, mx) = match (v.iter().min(), v.iter().max()) {
        (Some(a), Some(b)) => (*a, *b),
        _ => (0, 0),
    };
    format!("{{\"ok\":true,\"min\":{},\"max\":{}}}", mn, mx)
}
pub fn median(text: &str) -> String {
    let mut v: Vec<f64> = text.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let m = if v.is_empty() { 0.0 } else if v.len() % 2 == 1 { v[v.len() / 2] } else { (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0 };
    format!("{{\"ok\":true,\"median\":{:.4}}}", m)
}
pub fn is_prime(n: u64) -> String {
    let p = n >= 2 && (2..).take_while(|&d| d * d <= n).all(|d| n % d != 0);
    format!("{{\"ok\":true,\"n\":{},\"prime\":{}}}", n, p)
}
pub fn factorial(n: u32) -> String {
    let n = n.min(20);
    let mut r: u128 = 1;
    for i in 2..=n { r *= i as u128; }
    format!("{{\"ok\":true,\"n\":{},\"factorial\":\"{}\"}}", n, r)
}
pub fn gcd(a: u64, b: u64) -> String {
    let mut x = a; let mut y = b;
    while y != 0 { let t = y; y = x % y; x = t; }
    format!("{{\"ok\":true,\"a\":{},\"b\":{},\"gcd\":{}}}", a, b, x)
}
pub fn lcm(a: u64, b: u64) -> String {
    if a == 0 || b == 0 { return format!("{{\"ok\":true,\"a\":{},\"b\":{},\"lcm\":0}}", a, b); }
    let mut x = a; let mut y = b;
    while y != 0 { let t = y; y = x % y; x = t; }
    format!("{{\"ok\":true,\"a\":{},\"b\":{},\"lcm\":{}}}", a, b, a / x * b)
}
pub fn power(base: u64, exp: u32) -> String {
    let r = base.pow(exp.min(18));
    format!("{{\"ok\":true,\"base\":{},\"exp\":{},\"power\":\"{}\"}}", base, exp, r)
}
pub fn percentage(text: &str) -> String {
    let v: Vec<f64> = text.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let total: f64 = v.iter().sum();
    let s: Vec<String> = v.iter().map(|x| format!("{:.2}", if total == 0.0 { 0.0 } else { x * 100.0 / total })).collect();
    format!("{{\"ok\":true,\"pcts\":[{}]}}", s.join(","))
}

// ===========================================================================
// R. 时间 / 日期 / 节奏
// ===========================================================================

pub fn uptime_seconds() -> String {
    let o = pfile("/proc/uptime").and_then(|s| s.split_whitespace().next().map(|x| x.to_string())).unwrap_or_default();
    format!("{{\"ok\":true,\"uptime_s\":\"{}\"}}", json::jesc(&o))
}
pub fn utc_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let s = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let y = 1970 + s / 31556952;
    format!("{{\"ok\":true,\"epoch\":{},\"approx_year\":{}}}", s, y)
}
pub fn iso_date() -> String {
    let o = cmd("date -u '+%Y-%m-%d %H:%M:%S %Z' 2>/dev/null");
    format!("{{\"ok\":true,\"utc\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn weekday() -> String {
    let o = cmd("date -u '+%A' 2>/dev/null");
    format!("{{\"ok\":true,\"day\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn quarter() -> String {
    let o = cmd("date -u '+%m' 2>/dev/null");
    let m: u32 = o.and_then(|x| x.parse().ok()).unwrap_or(1);
    format!("{{\"ok\":true,\"month\":{},\"quarter\":{}}}", m, (m - 1) / 3 + 1)
}
pub fn seconds_until(end_unix: i64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    format!("{{\"ok\":true,\"target\":{},\"now\":{},\"remaining\":{}}}", end_unix, now, end_unix - now)
}
pub fn calendar_seed() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let s = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("{{\"ok\":true,\"seed\":{}}}", s)
}
pub fn is_leap_year(year: i64) -> String {
    let ok = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    format!("{{\"ok\":true,\"year\":{},\"leap\":{}}}", year, ok)
}
pub fn day_count_month() -> String {
    let o = cmd("date -u '+%Y %m' 2>/dev/null");
    let v: Vec<u32> = o.unwrap_or_default().split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let (y, m) = if v.len() >= 2 { (v[0] as i64, v[1]) } else { (2024, 1) };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let d = match m { 2 => if leap { 29 } else { 28 }, 4 | 6 | 9 | 11 => 30, _ => 31 };
    format!("{{\"ok\":true,\"year\":{},\"month\":{},\"days\":{}}}", y, m, d)
}
pub fn time_signed_bin() -> String {
    let o = cmd("date +%s 2>/dev/null");
    let s: i64 = o.and_then(|x| x.parse().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"unix\":{},\"hi_bytes\":\"{:08x}\"}}", s, (s as u64) >> 24)
}

// ===========================================================================
// S. 进程 / 系统字段 / 目录
// ===========================================================================

pub fn pid_count_all() -> String {
    let n = cmd("ls /proc | grep -E '^[0-9]+$' | wc -l").and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"pids\":{}}}", n)
}
pub fn process_start(pid: u32) -> String {
    let o = cmd(&format!("ps -o lstart= -p {} 2>/dev/null", pid));
    format!("{{\"ok\":true,\"pid\":{},\"started\":\"{}\"}}", pid, json::jesc(&o.unwrap_or_default()))
}
pub fn process_rss(pid: u32) -> String {
    let kb = num(&format!("/proc/{}/status", pid), "VmRSS:");
    format!("{{\"ok\":true,\"pid\":{},\"rss_mb\":{:.1}}}", pid, kb as f64 / 1024.0)
}
pub fn process_vsz(pid: u32) -> String {
    let kb = num(&format!("/proc/{}/status", pid), "VmSize:");
    format!("{{\"ok\":true,\"pid\":{},\"vsz_mb\":{:.1}}}", pid, kb as f64 / 1024.0)
}
pub fn process_state(pid: u32) -> String {
    let o = pfile(&format!("/proc/{}/stat", pid)).and_then(|s| s.split_whitespace().nth(2).map(|x| x.to_string())).unwrap_or_default();
    format!("{{\"ok\":true,\"pid\":{},\"state\":\"{}\"}}", pid, json::jesc(&o))
}
pub fn open_files(pid: u32) -> String {
    let n = cmd(&format!("ls /proc/{}/fd 2>/dev/null | wc -l", pid)).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    format!("{{\"ok\":true,\"pid\":{},\"fds\":{}}}", pid, n)
}
pub fn io_by_pid(pid: u32) -> String {
    let o = cmd(&format!("awk 'NR>=1{{print}}' /proc/{}/io 2>/dev/null | head -8", pid));
    format!("{{\"ok\":true,\"pid\":{},\"io\":\"{}\"}}", pid, json::jesc(&o.unwrap_or_default()))
}
pub fn longest_cmdline() -> String {
    let o = cmd("ps -eo cmd --sort=-args 2>/dev/null | head -2 | tail -1");
    format!("{{\"ok\":true,\"longest\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn thread_total() -> String {
    let o = cmd("ps -eLf 2>/dev/null | tail -n +2 | wc -l");
    format!("{{\"ok\":true,\"threads\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}
pub fn dir_nlink(path: &str) -> String {
    let n = cmd(&format!("stat -c '%h' {} 2>/dev/null", shq(path)));
    format!("{{\"ok\":true,\"path\":\"{}\",\"nlink\":\"{}\"}}", json::jesc(path), json::jesc(&n.unwrap_or_default()))
}

// ===========================================================================
// T. 杂项 / 实用 / 校验
// ===========================================================================

pub fn tar_list(file: &str) -> String {
    let o = cmd(&format!("tar -tzf {} 2>/dev/null | head -30", shq(file)));
    format!("{{\"ok\":true,\"file\":\"{}\",\"entries\":\"{}\"}}", json::jesc(file), json::jesc(&o.unwrap_or_default()))
}
pub fn gz_info(file: &str) -> String {
    let o = cmd(&format!("gzip -l {} 2>/dev/null", shq(file)));
    format!("{{\"ok\":true,\"file\":\"{}\",\"info\":\"{}\"}}", json::jesc(file), json::jesc(&o.unwrap_or_default()))
}
pub fn sha256sum_file(file: &str) -> String {
    let (o, _) = cmd_e(&format!("sha256sum {} 2>/dev/null", shq(file)));
    let sum = o.split_whitespace().next().unwrap_or("");
    format!("{{\"ok\":true,\"file\":\"{}\",\"sha256\":\"{}\"}}", json::jesc(file), sum)
}
pub fn env_all() -> String {
    let mut e: Vec<String> = std::env::vars().map(|(k, v)| format!("{}={}", k, v)).collect();
    e.sort();
    format!("{{\"ok\":true,\"env\":\"{}\"}}", json::jesc(&e.join("\n")))
}
pub fn echo_args(text: &str) -> String {
    format!("{{\"ok\":true,\"echo\":\"{}\"}}", json::jesc(text))
}
pub fn len_bytes(text: &str) -> String {
    format!("{{\"ok\":true,\"bytes\":{}}}", text.len())
}
pub fn is_numeric(text: &str) -> String {
    let ok = text.trim().parse::<f64>().is_ok();
    format!("{{\"ok\":true,\"numeric\":{}}}", ok)
}
pub fn to_int(text: &str) -> String {
    let v = text.trim().split('.').next().unwrap_or("").parse::<i64>().unwrap_or(0);
    format!("{{\"ok\":true,\"int\":{}}}", v)
}
pub fn byte_units(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64; let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 { v /= 1024.0; i += 1; }
    format!("{{\"ok\":true,\"bytes\":{},\"human\":\"{:.2} {}\"}}", bytes, v, units[i])
}
pub fn is_systemd() -> String {
    let o = cmd("test -d /run/systemd/system && echo yes || echo no");
    format!("{{\"ok\":true,\"systemd\":\"{}\"}}", json::jesc(&o.unwrap_or_default()))
}