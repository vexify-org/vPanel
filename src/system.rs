//! 系统监控：后台采样线程采集 CPU / 网络曲线（有界环形缓冲），
//! 以及按需读取 /proc 得到系统快照与进程列表。
//!
//! 设计要点：曲线历史用 60 点的环形缓冲，内存恒定有限；
//! 系统快照与进程列表在每次请求时现场读取，随请求结束即释放，常驻内存不受影响。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 重启面板自身（对齐 iotapanel 的 /api/system/restart）：
/// 从 /proc/self/cmdline 重建启动参数，挂一个延迟 exec 的后台进程后当前进程退出。
pub fn self_restart() -> (bool, String) {
    let cmdline = std::fs::read("/proc/self/cmdline").unwrap_or_default();
    let argv: Vec<String> = cmdline
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect();
    if argv.is_empty() {
        return (false, "无法读取启动参数".to_string());
    }
    let quoted: Vec<String> = argv
        .iter()
        .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
        .collect();
    let cmd = format!("(sleep 0.3; exec {} >/dev/null 2>&1 </dev/null &)", quoted.join(" "));
    let ok = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()
        .map(|_| true)
        .unwrap_or(false);
    if ok {
        (true, "重启中…请稍后刷新".to_string())
    } else {
        (false, "启动重启失败".to_string())
    }
}

/// 实时监控器，由后台线程每 1s 采样更新。
pub struct Monitor {
    pub cpu: Mutex<VecDeque<f32>>,        // CPU 使用率 % 历史
    pub net_dn: Mutex<VecDeque<u64>>,     // 下行速率 B/s 历史
    pub net_up: Mutex<VecDeque<u64>>,     // 上行速率 B/s 历史
    last_cpu: Mutex<(u64, u64)>,          // 上一次 cpu 的 (total, idle)
    last_net: Mutex<(u64, u64)>,          // 上一次网络的 (recv, trans)
    ring: usize,
}

const RING: usize = 60;
/// 后台线程栈上限：监控/调度线程只有浅调用，256KB 足够，可压低栈虚拟保留。
const STACK_KB: usize = 256 * 1024;

impl Monitor {
    fn new() -> Arc<Monitor> {
        let m = Arc::new(Monitor {
            cpu: Mutex::new(VecDeque::with_capacity(RING)),
            net_dn: Mutex::new(VecDeque::with_capacity(RING)),
            net_up: Mutex::new(VecDeque::with_capacity(RING)),
            last_cpu: Mutex::new((0, 0)),
            last_net: Mutex::new((0, 0)),
            ring: RING,
        });
        m
    }

    /// 启动后台采样线程，返回共享监控器。
    pub fn start() -> Arc<Monitor> {
        let m = Monitor::new();
        let handle = Arc::downgrade(&m);
        let m2 = m.clone();
        std::thread::Builder::new()
            .stack_size(STACK_KB)
            .name("mon".into())
            .spawn(move || {
            // 每 5 个采样写一条磁盘历史（约 5s），复用同一线程免去额外常驻线程。
            let mut tick: u32 = 0;
            loop {
                // 仅当主监控仍存活时继续采样。
                if handle.strong_count() == 0 {
                    break;
                }
                let _ = m2.sample();
                tick = tick.wrapping_add(1);
                if tick % 5 == 0 {
                    crate::monitor::write_history();
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        m
    }

    fn sample(&self) {
        // ---- CPU ----
        if let Some((total, idle)) = read_cpu() {
            let mut last = self.last_cpu.lock().unwrap();
            let usage = if last.0 > 0 && total > last.0 {
                let t = (total - last.0) as f32;
                let i = (idle - last.1) as f32;
                if t <= 0.0 {
                    0.0
                } else {
                    ((t - i) / t * 100.0).clamp(0.0, 100.0)
                }
            } else {
                0.0
            };
            *last = (total, idle);
            push_ring(&self.cpu, usage, self.ring);
        }

        // ---- 网络 ----
        if let Some((recv, trans)) = read_net() {
            let mut last = self.last_net.lock().unwrap();
            let (dn, up) = if last.0 > 0 && recv >= last.0 && trans >= last.1 {
                (recv - last.0, trans - last.1)
            } else {
                (0, 0)
            };
            *last = (recv, trans);
            push_ring(&self.net_dn, dn, self.ring);
            push_ring(&self.net_up, up, self.ring);
        }
    }

    /// 把历史序列转成 JSON 数组字符串（保留一位小数 / 原值）。
    pub fn series_json(&self) -> (String, String, String) {
        let cpu = self.cpu.lock().unwrap();
        let dn = self.net_dn.lock().unwrap();
        let up = self.net_up.lock().unwrap();
        (
            floats_to_json(&cpu),
            uints_to_json(&dn),
            uints_to_json(&up),
        )
    }
}

fn push_ring<T: Clone>(q: &Mutex<VecDeque<T>>, v: T, ring: usize) {
    let mut g = q.lock().unwrap();
    if g.len() >= ring {
        g.pop_front();
    }
    g.push_back(v);
}

fn floats_to_json(v: &VecDeque<f32>) -> String {
    let mut s = String::new();
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{:.1}", x));
    }
    s.push(']');
    s
}

fn uints_to_json(v: &VecDeque<u64>) -> String {
    let mut s = String::new();
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&x.to_string());
    }
    s.push(']');
    s
}

fn read_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// /proc/stat 第一行 -> (总量, 空闲)。
fn read_cpu() -> Option<(u64, u64)> {
    let s = read_file("/proc/stat")?;
    let line = s.lines().next()?;
    let mut vals = line.split_whitespace().skip(1).map(|x| x.parse::<u64>().ok().unwrap_or(0));
    let user = vals.next()?;
    let nice = vals.next()?;
    let system = vals.next()?;
    let idle = vals.next()?;
    let iowait = vals.next()?;
    let irq = vals.next()?;
    let softirq = vals.next()?;
    let steal = vals.next()?;
    let total = user + nice + system + idle + iowait + irq + softirq + steal;
    Some((total, idle + iowait))
}

/// /proc/net/dev 求和 -> (接收字节, 发送字节)。
fn read_net() -> Option<(u64, u64)> {
    let s = read_file("/proc/net/dev")?;
    let mut recv = 0u64;
    let mut trans = 0u64;
    for line in s.lines().skip(2) {
        let mut it = line.split(':');
        it.next()?; // interface
        let rest = it.next()?;
        let mut f = rest.split_whitespace().map(|x| x.parse::<u64>().ok().unwrap_or(0));
        if let Some(r) = f.next() {
            recv += r;
        }
        // 第 9 个字段是发送字节。
        let mut trans_v = 0u64;
        for _ in 0..8 {
            if let Some(_) = f.next() {
                continue;
            }
            break;
        }
        if let Some(t) = f.next() {
            trans_v = t;
        }
        trans += trans_v;
    }
    Some((recv, trans))
}

/// CPU 核数与型号。
pub fn cpu_info() -> (usize, String) {
    let mut count = 0usize;
    let mut model = String::new();
    if let Some(s) = read_file("/proc/cpuinfo") {
        for line in s.lines() {
            if let Some(v) = line.strip_prefix("processor") {
                if v.trim().starts_with(':') {
                    count += 1;
                }
            } else if let Some(v) = line.strip_prefix("model name") {
                if let Some(idx) = v.find(':') {
                    if model.is_empty() {
                        model = v[idx + 1..].trim().to_string();
                    }
                }
            }
        }
    }
    if count == 0 {
        count = 1;
    }
    (count, model)
}

/// 内存快照 (总, 可用)，单位 byte。
pub fn mem() -> Option<(u64, u64)> {
    let s = read_file("/proc/meminfo")?;
    let mut total = 0u64;
    let mut avail = 0u64;
    for line in s.lines() {
        if let Some(v) = line.strip_prefix("MemTotal:") {
            total = parse_kb(v);
        } else if let Some(v) = line.strip_prefix("MemAvailable:") {
            avail = parse_kb(v);
        }
    }
    if total == 0 {
        return None;
    }
    Some((total, avail))
}

fn parse_kb(s: &str) -> u64 {
    s.trim()
        .split_whitespace()
        .next()
        .and_then(|x| x.parse().ok())
        .unwrap_or(0)
        * 1024
}

/// 系统快照 -> JSON 字符串。
pub fn system_json(m: &Monitor) -> String {
    let host = read_file("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let (cores, model) = cpu_info();
    let uptime = read_file("/proc/uptime")
        .and_then(|s| s.split_whitespace().next().map(|x| x.parse::<u64>().ok().unwrap_or(0)))
        .unwrap_or(0);
    let load = read_file("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).map(str::to_string).collect::<Vec<_>>())
        .unwrap_or_default();
    let (total, avail) = mem().unwrap_or((0, 0));
    let used = total.saturating_sub(avail);
    let (cpu_s, net_dn, net_up) = m.series_json();
    let disk = disk_json();

    format!(
        "{{\"host\":\"{}\",\"model\":\"{}\",\"cores\":{},\"uptime\":{},\"load\":[{}],\"mem\":{{\"total\":{},\"used\":{},\"free\":{},\"used_pct\":{:.1}}},\"disk\":{},\"cpu\":{},\"net\":{{\"down\":{},\"up\":{}}},\"series\":{{\"cpu\":{},\"net_dn\":{},\"net_up\":{}}}}}",
        crate::json::jesc(&host),
        crate::json::jesc(&model),
        cores,
        uptime,
        load.join(","),
        total,
        used,
        avail,
        if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 },
        disk,
        current_cpu(), // 当前 CPU 使用率由最近两次采样算出
        // 最新一条网络速率
        m.net_dn.lock().unwrap().back().copied().unwrap_or(0),
        m.net_up.lock().unwrap().back().copied().unwrap_or(0),
        cpu_s,
        net_dn,
        net_up,
    )
}

/// 现场取两次 /proc/stat 之差得到当前 CPU 使用率（间隔约 60ms）。
fn current_cpu() -> f32 {
    let first = read_cpu();
    std::thread::sleep(Duration::from_millis(60));
    if let (Some((t0, i0)), Some((t1, i1))) = (first, read_cpu()) {
        if t1 > t0 {
            let tt = (t1 - t0) as f32;
            let ii = (i1 - i0) as f32;
            if tt > 0.0 {
                return ((tt - ii) / tt * 100.0).clamp(0.0, 100.0);
            }
        }
    }
    0.0
}

/// 磁盘使用（df -kP）-> JSON 数组。
pub fn disk_json() -> String {
    let mut out = String::from("[");
    let mut first = true;
    if let Some(out_str) = crate::json::run_out("df", &["-kP"]) {
        for line in out_str.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 6 {
                let fs = parts[0];
                let mount = parts[5];
                if !mount.starts_with('/') {
                    continue;
                }
                let (sz, used, avail, pct) = (
                    parts[1].parse::<u64>().unwrap_or(0) * 1024,
                    parts[2].parse::<u64>().unwrap_or(0) * 1024,
                    parts[3].parse::<u64>().unwrap_or(0) * 1024,
                    parts[4].replace('%', "").parse::<u32>().unwrap_or(0),
                );
                if !first {
                    out.push(',');
                }
                first = false;
                out.push_str(&format!(
                    "{{\"fs\":\"{}\",\"mount\":\"{}\",\"total\":{},\"used\":{},\"free\":{},\"pct\":{}}}",
                    crate::json::jesc(fs),
                    crate::json::jesc(mount),
                    sz,
                    used,
                    avail,
                    pct
                ));
            }
        }
    }
    out.push(']');
    out
}

/// 读写 /proc/* 得到进程列表 -> JSON 字符串（按 RSS 排序，最多 80 条）。
pub fn processes_json() -> String {
    let mut list = Vec::new();
    let dir = std::path::Path::new("/proc");
    for entry in std::fs::read_dir(dir).ok().into_iter().flatten() {
        let path = entry.ok().map(|e| e.path());
        let name = path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
        let name = match name {
            Some(n) => n,
            None => continue,
        };
        if !name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let pid: u32 = match name.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if let Some((comm, state, rss)) = proc_meta(&path.unwrap()) {
            list.push((rss, pid, comm, state));
        }
    }
    list.sort_by(|a, b| b.0.cmp(&a.0));
    list.truncate(80);

    let mut out = String::from("{\"len\":");
    out.push_str(&list.len().to_string());
    out.push_str(",\"list\":[");
    for (i, (rss, pid, comm, state)) in list.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"pid\":{},\"name\":\"{}\",\"state\":\"{}\",\"rss\":{}}}",
            pid,
            crate::json::jesc(comm),
            crate::json::jesc(state),
            rss
        ));
    }
    out.push_str("]}");
    out
}

/// 读取一个进程的 (名称, 状态, RSS字节)。
fn proc_meta(path: &std::path::Path) -> Option<(String, String, u64)> {
    let comm = read_file(&path.join("comm").to_string_lossy())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    // stat：以括号为界的 (comm) 之后第 1 个字段是 state。
    let state = read_file(&path.join("stat").to_string_lossy())
        .and_then(|s| {
            let end = s.rfind(')')?;
            let rest = s[end + 1..].trim();
            rest.split_whitespace().next().map(|x| x.to_string())
        })
        .unwrap_or_default();
    let rss = read_file(&path.join("status").to_string_lossy())
        .and_then(|s| {
            s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| {
                l.trim().trim_start_matches("VmRSS:").trim().split_whitespace().next()
                    .and_then(|x| x.parse::<u64>().ok())
            })
        })
        .map(|kb| kb * 1024)
        .unwrap_or(0);
    if comm.is_empty() {
        None
    } else {
        Some((comm, state, rss))
    }
}

/// 结束进程：向 pid 发送 SIGKILL。返回是否成功。
pub fn kill_pid(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}