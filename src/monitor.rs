//! 监控历史（对标宝塔）：
//! 后台线程每 5s 采样一次 cpu/mem/net/disk，追加到磁盘 `.vpanel-monitor.jsonl`，
//! 供 `/api/monitor` 查询趋势。文件有界，超过上限自动裁剪最旧的记录。

use std::time::Duration;

const HIST_FILE: &str = ".vpanel-monitor.jsonl";
/// 文件最多保留的行数（约 5s×20000 ≈ 27 小时的采样）。
const MAX_LINES: usize = 20000;

fn hist_file() -> String {
    format!("{}/{}", crate::config::Config::panel_dir(), HIST_FILE)
}

/// 启动后台采样线程（每 5s 一笔）。
pub fn start() {
    std::thread::spawn(|| loop {
        let (dn, up) = sample_net();
        append(now_ts(), sample_cpu(), sample_mem(), dn, up, sample_disk_pct());
        std::thread::sleep(Duration::from_secs(5));
    });
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 采一条指标并追加；越界则裁剪。
fn append(ts: u64, cpu: f32, mem_pct: u32, dn: u64, up: u64, disk_pct: u32) {
    let line = format!(
        "{{\"t\":{},\"cpu\":{:.1},\"mem\":{},\"dn\":{},\"up\":{},\"disk\":{}}}\n",
        ts, cpu, mem_pct, dn, up, disk_pct
    );
    let f = hist_file();
    let mut content = match std::fs::read_to_string(&f) {
        Ok(s) => s,
        Err(_) => String::new(),
    };
    content.push_str(&line);
    // 有界裁剪：只保留最后 MAX_LINES 行。
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() > MAX_LINES {
        content = lines[lines.len() - MAX_LINES..]
            .iter()
            .map(|l| format!("{}\n", l))
            .collect();
    }
    let _ = std::fs::write(&f, content.as_bytes());
}

/// CPU 使用率 %：取两次 /proc/stat 之差。
fn sample_cpu() -> f32 {
    let first = read_cpu();
    std::thread::sleep(Duration::from_millis(80));
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

fn read_cpu() -> Option<(u64, u64)> {
    let s = std::fs::read_to_string("/proc/stat").ok()?;
    let line = s.lines().next()?;
    let mut it = line.split_whitespace().skip(1);
    let user: u64 = it.next()?.parse().ok()?;
    let nice: u64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let system: u64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let idle: u64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let iowait: u64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let total = user + nice + system + idle + iowait;
    Some((total, idle + iowait))
}

/// 内存使用率 %。
fn sample_mem() -> u32 {
    let s = match std::fs::read_to_string("/proc/meminfo") {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let mut total = 0u64;
    let mut avail = 0u64;
    for line in s.lines() {
        if let Some(v) = line.strip_prefix("MemTotal:") {
            total = kb(v);
        } else if let Some(v) = line.strip_prefix("MemAvailable:") {
            avail = kb(v);
        }
    }
    if total == 0 {
        0
    } else {
        ((total - avail) * 100 / total) as u32
    }
}

fn kb(s: &str) -> u64 {
    s.trim()
        .split_whitespace()
        .next()
        .and_then(|x| x.parse().ok())
        .unwrap_or(0)
        * 1024
}

/// 全网口收发速率 B/s。
fn sample_net() -> (u64, u64) {
    let first = net_bytes();
    std::thread::sleep(Duration::from_millis(300));
    if let (Some((r0, t0)), Some((r1, t1))) = (first, net_bytes()) {
        return ((r1.saturating_sub(r0)) / 300, (t1.saturating_sub(t0)) / 300);
    }
    (0, 0)
}

fn net_bytes() -> Option<(u64, u64)> {
    let s = std::fs::read_to_string("/proc/net/dev").ok()?;
    let mut recv = 0u64;
    let mut trans = 0u64;
    for line in s.lines().skip(2) {
        let mut it = line.split(':');
        let _if = it.next()?;
        let rest = it.next()?;
        let fe: Vec<u64> = rest.split_whitespace().map(|x| x.parse().ok().unwrap_or(0)).collect();
        if !fe.is_empty() {
            recv += fe[0];
        }
        if fe.len() >= 9 {
            trans += fe[8];
        }
    }
    Some((recv, trans))
}

/// 根分区使用率 %（df 输出里的挂载点 /）。
fn sample_disk_pct() -> u32 {
    let out = std::process::Command::new("df").args(["-kP", "/"]).output();
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout);
        for line in s.lines().skip(1) {
            let p: Vec<&str> = line.split_whitespace().collect();
            if p.len() >= 5 {
                if let Ok(v) = p[4].replace('%', "").parse::<u32>() {
                    return v;
                }
            }
        }
    }
    0
}

/// 趋势查询：返回最近 n 条（由旧到新），每条约 5s。
pub fn monitor_json(n: usize) -> String {
    let n = if n == 0 { 120 } else { n.min(10000) };
    let data = std::fs::read_to_string(hist_file()).unwrap_or_default();
    let mut lines: Vec<&str> = data.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() > n {
        lines = lines[lines.len() - n..].to_vec();
    }
    format!("{{\"ok\":true,\"interval\":5,\"count\":{},\"series\":[{}]}}", lines.len(), lines.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_positive() {
        assert!(now_ts() > 1_500_000_000);
    }

    #[test]
    fn mem_or_net_no_panic() {
        // 采样函数在任何环境下都不应 panic。
        let _ = sample_mem();
        let _ = sample_net();
        let _ = sample_disk_pct();
    }
}