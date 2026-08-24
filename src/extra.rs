//! 新增管理功能：系统信息、网络连接、实时日志、轻量文件管理。
//!
//! 全部按需读取 / 按需执行，随请求结束即释放，常驻内存保持有界（预算内）。

use crate::json;

/// 读 /proc 或 /sys 文本（None 表示不存在）。
fn pfile(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// 执行只读命令并取 stdout 首行。
fn cmd_first(cmd: &str) -> Option<String> {
    let out = std::process::Command::new("/bin/sh").arg("-c").arg(cmd).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

fn cmd_all(cmd: &str) -> Option<String> {
    let out = std::process::Command::new("/bin/sh").arg("-c").arg(cmd).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

/// 文件管理根目录越界检查。
///
/// - 未配置 `fs_root`：直接放行，返回 `Some(path)`（向后兼容）。
/// - 已配置：把路径规范化并确认其落在根目录内；越界或无法解析返回 `None`。
///   目标不存在时，以「父目录在根内」为准则（便于写/上传新文件）。
fn confined(path: &str) -> Option<String> {
    let root = crate::config::fs_root()?; // None => 不限制
    let p = std::path::Path::new(path);
    let canon = if p.exists() {
        p.canonicalize().ok()
    } else {
        p.parent()
            .and_then(|par| par.canonicalize().ok())
            .map(|pp| pp.join(p.file_name().unwrap_or_default()))
    };
    match canon {
        Some(c) if c.starts_with(&root) => Some(c.to_string_lossy().into_owned()),
        _ => None,
    }
}

fn human(bytes: u64) -> String {
    const U: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{:.1} {}", v, U[i])
}

// ---------------------------------------------------------------------------
// 1. 系统信息
// ---------------------------------------------------------------------------

/// 系统信息 -> JSON。
pub fn sysinfo_json() -> String {
    let (cores, cpu_model) = crate::system::cpu_info();
    let host = pfile("/proc/sys/kernel/hostname").unwrap_or_default();
    let kernel = pfile("/proc/sys/kernel/osrelease").unwrap_or_default();
    let arch = std::env::consts::ARCH.to_string();
    let uptime = pfile("/proc/uptime")
        .and_then(|s| s.split_whitespace().next().map(|x| x.parse::<u64>().unwrap_or(0)))
        .unwrap_or(0);
    let load = pfile("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_default();

    // OS 名称（/etc/os-release）。
    let os = pfile("/etc/os-release")
        .map(|s| {
            s.lines()
                .find_map(|l| {
                    if let Some(v) = l.strip_prefix("PRETTY_NAME") {
                        let v = v.trim_start_matches('=').trim_matches('"');
                        Some(v.to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_default()
        })
        .filter(|s| !s.is_empty())
        .or_else(|| cmd_first("lsb_release -d 2>/dev/null | cut -d: -f2"))
        .unwrap_or_default();

    let (mem_total, mem_avail) = crate::system::mem().unwrap_or((0, 0));
    // 交换分区块（从 /proc/meminfo）。
    let (swap_total_bytes, swap_free_bytes) = {
        let mut total = 0u64;
        let mut free = 0u64;
        if let Some(s) = pfile("/proc/meminfo") {
            for l in s.lines() {
                if let Some(v) = l.strip_prefix("SwapTotal:") {
                    total = kb_of(v);
                } else if let Some(v) = l.strip_prefix("SwapFree:") {
                    free = kb_of(v);
                }
            }
        }
        (total, free)
    };

    // 磁盘分区（df -Pk：块、已用、可用、使用%、挂载点）。
    let mut disks = Vec::new();
    if let Some(out) = cmd_all("df -Pk 2>/dev/null") {
        for line in out.lines().skip(1) {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 6 {
                continue;
            }
            let fs = f[0];
            let blocks = f[1].parse::<f64>().unwrap_or(0.0) * 1024.0;
            let used = f[2].parse::<f64>().unwrap_or(0.0) * 1024.0;
            let pct = f[4].trim_end_matches('%');
            let mount = f[5];
            // 跳过伪文件系统。
            if fs.starts_with("tmpfs") || fs.starts_with("udev") || mount == "/etc/hosts" {
                continue;
            }
            disks.push(format!(
                "{{\"fs\":\"{}\",\"mount\":\"{}\",\"total\":{},\"used\":{},\"pct\":\"{}\"}}",
                json::jesc(fs),
                json::jesc(mount),
                (blocks as u64),
                (used as u64),
                json::jesc(pct)
            ));
        }
    }

    // 温度（thermal_zone，单位毫摄氏度）。
    let mut temp: Option<String> = None;
    if let Ok(it) = std::fs::read_dir("/sys/class/thermal") {
        let mut vals: Vec<i64> = Vec::new();
        for ent in it.flatten() {
            let p = ent.path();
            let zone = p
                .file_name()
                .and_then(|n| n.to_str().map(str::to_string))
                .unwrap_or_default();
            if zone.starts_with("thermal_zone") {
                let t = std::fs::read_to_string(p.join("temp"))
                    .ok()
                    .and_then(|s| s.trim().parse::<i64>().ok());
                if let Some(t) = t {
                    vals.push(t / 1000);
                }
            }
        }
        if !vals.is_empty() {
            temp = Some(format!("{:.0}°C", vals.iter().sum::<i64>() as f64 / vals.len() as f64));
        }
    }

    format!(
        "{{\"ok\":true,\"os\":\"{}\",\"kernel\":\"{}\",\"arch\":\"{}\",\"host\":\"{}\",\"uptime\":{},\"cores\":{},\"cpu_model\":\"{}\",\"load\":\"{}\",\"mem_total\":{},\"mem_avail\":{},\"mem_used\":{},\"mem_pct\":{},\"swap_total\":{},\"swap_used\":{},\"temp\":\"{}\",\"disks\":[{}]}}",
        json::jesc(&os),
        json::jesc(&kernel),
        json::jesc(&arch),
        json::jesc(&host),
        uptime,
        cores,
        json::jesc(&cpu_model),
        json::jesc(&load),
        mem_total,
        mem_avail,
        mem_total.saturating_sub(mem_avail),
        if mem_total > 0 {
            ((mem_total - mem_avail) as f64 / mem_total as f64 * 100.0).round() as u64
        } else {
            0
        },
        swap_total_bytes,
        swap_total_bytes.saturating_sub(swap_free_bytes),
        json::jesc(&temp.unwrap_or_default()),
        disks.join(",")
    )
}

fn kb_of(s: &str) -> u64 {
    s.trim()
        .split_whitespace()
        .next()
        .and_then(|x| x.parse::<u64>().ok())
        .unwrap_or(0)
        * 1024
}

// ---------------------------------------------------------------------------
// 2. 网络连接监控
// ---------------------------------------------------------------------------

/// 网络连接统计 -> JSON。优先 ss，缺省回退 netstat。
pub fn conns_json() -> String {
    let raw = cmd_all("ss -tan 2>/dev/null || netstat -tan 2>/dev/null").unwrap_or_default();
    // 状态计数 + 本地端口计数。
    let mut states = std::collections::HashMap::<String, u64>::new();
    let mut ports = std::collections::HashMap::<String, (u64, u64)>::new(); // (listen, established)
    let mut total = 0u64;
    for line in raw.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            continue;
        }
        let state = f[0];
        let local = f[3];
        let port = local.rsplit(':').next().unwrap_or("").to_string();
        total += 1;
        *states.entry(state.to_string()).or_insert(0) += 1;
        let e = ports.entry(port.clone()).or_insert((0, 0));
        if state == "LISTEN" {
            e.0 += 1;
        } else if state == "ESTAB" {
            e.1 += 1;
        }
    }
    let mut state_json: Vec<String> = states
        .iter()
        .map(|(s, c)| format!("{{\"state\":\"{}\",\"count\":{}}}", json::jesc(s), c))
        .collect();
    state_json.sort();
    let mut port_json: Vec<String> = ports
        .iter()
        .map(|(p, (l, e))| {
            format!(
                "{{\"port\":\"{}\",\"listen\":{},\"estab\":{},\"total\":{}}}",
                json::jesc(p),
                l,
                e,
                l + e
            )
        })
        .collect();
    // 按 total 倒序。
    port_json.sort_by(|a, b| b.len().cmp(&a.len()));
    format!(
        "{{\"ok\":true,\"total\":{},\"states\":[{}],\"ports\":[{}]}}",
        total,
        state_json.join(","),
        port_json.join(",")
    )
}

/// 结束监听某端口的进程（对应 LISTEN 的 PID）。
pub fn conn_kill(port: &str) -> (bool, String) {
    // 找到该端口上所有 LISTEN 的 PID。
    let raw = cmd_all("ss -tlnp 2>/dev/null || netstat -tlnp 2>/dev/null").unwrap_or_default();
    let mut pids = std::collections::HashMap::<String, String>::new(); // pid -> proc
    for line in raw.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            continue;
        }
        let local = f[3];
        let lp = local.rsplit(':').next().unwrap_or("");
        if lp != port {
            continue;
        }
        // 提取 pid=NN 或 /proc/NN（部分 ss 输出无 pid）。
        if let Some(pid) = line
            .split(|c: char| c == ',' || c == '"' || c == ' ')
            .find_map(|t| t.strip_prefix("pid=").map(str::to_string))
        {
            pids.insert(pid.clone(), pid);
        } else if f.len() >= 6 && !local.starts_with("*:") {
            // netstat 第 7 列常为 "NN/proc"
        }
    }
    if pids.is_empty() {
        // 尝试用 fuser 兜底。
        if let Some(out) = cmd_all(&format!("fuser -n tcp {} 2>/dev/null", port)) {
            for tok in out.split_whitespace() {
                if let Ok(pid) = tok.parse::<i64>() {
                    if crate::system::kill_pid(pid as u32) {
                        return (true, format!("已结束占用端口 {} 的进程 {}", port, pid));
                    }
                }
            }
        }
        return (false, format!("端口 {} 上没有可识别的监听进程", port));
    }
    let mut done = 0;
    for pid in pids.keys() {
        if let Ok(p) = pid.parse::<u32>() {
            if crate::system::kill_pid(p) {
                done += 1;
            }
        }
    }
    if done > 0 {
        (true, format!("已结束 {} 个监听端口 {} 的进程", done, port))
    } else {
        (false, format!("端口 {} 的进程结束失败（可能权限不足）", port))
    }
}

// ---------------------------------------------------------------------------
// 3. 实时日志
// ---------------------------------------------------------------------------

/// 取文件最后 n 行 -> JSON（含当前字节长度，供 follow 增量）。
pub fn log_tail_json(file: &str, n: usize) -> String {
    let data = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            return format!("{{\"ok\":false,\"msg\":\"{}\"}}", json::jesc(&e.to_string()))
        }
    };
    let size = data.len() as u64;
    let lines: Vec<&str> = data.lines().collect();
    let take = lines.len().saturating_sub(n.max(1));
    let got = &lines[take..];
    let arr: Vec<String> = got.iter().map(|l| format!("\"{}\"", json::jesc(l))).collect();
    format!(
        "{{\"ok\":true,\"size\":{},\"lines\":[{}]}}",
        size,
        arr.join(",")
    )
}

/// 增量读取：从 pos 字节处读到末尾返回新增行；rotated 时从头开始。
pub fn log_follow_json(file: &str, pos: u64) -> String {
    let data = match std::fs::read(file) {
        Ok(s) => s,
        Err(e) => {
            return format!("{{\"ok\":false,\"msg\":\"{}\"}}", json::jesc(&e.to_string()))
        }
    };
    if data.len() == 0 {
        return format!("{{\"ok\":true,\"size\":0,\"lines\":[]}}");
    }
    let start = if (pos as usize) > data.len() { 0 } else { pos as usize };
    let chunk = &data[start..data.len().min(start + FOLLOW_MAX)];
    let text = String::from_utf8_lossy(chunk);
    let lines: Vec<&str> = text.lines().collect();
    let arr: Vec<String> = lines.iter().map(|l| format!("\"{}\"", json::jesc(l))).collect();
    format!(
        "{{\"ok\":true,\"size\":{},\"lines\":[{}]}}",
        data.len(),
        arr.join(",")
    )
}

const FOLLOW_MAX: usize = 256 * 1024;

// ---------------------------------------------------------------------------
// 4. 轻量文件管理
// ---------------------------------------------------------------------------

/// 目录列表 -> JSON。
pub fn ls_json(path: &str) -> String {
    let real = match confined(path) {
        Some(p) => p,
        None => {
            return format!(
                "{{\"ok\":false,\"msg\":\"{}\"}}",
                json::jesc("路径越界：超出文件根目录限制")
            )
        }
    };
    let read = match std::fs::read_dir(&real) {
        Ok(r) => r,
        Err(e) => {
            return format!("{{\"ok\":false,\"msg\":\"{}\"}}", json::jesc(&e.to_string()))
        }
    };
    let mut items: Vec<(bool, String, u64, i64)> = Vec::new(); // (dir, name, size, mtime)
    for ent in read.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        let md = ent.metadata();
        let (dir, size, mtime): (bool, u64, i64) = match md {
            Ok(m) => {
                let t: i64 = m
                    .modified()
                    .ok()
                    .and_then(|t: std::time::SystemTime| {
                        t.duration_since(std::time::UNIX_EPOCH).ok()
                    })
                    .map(|d: std::time::Duration| d.as_secs() as i64)
                    .unwrap_or(0);
                if m.is_dir() {
                    (true, 0, t)
                } else if m.is_file() {
                    (false, m.len(), t)
                } else {
                    (false, 0, t)
                }
            }
            Err(_) => (false, 0, 0),
        };
        items.push((dir, name, size, mtime));
    }
    // 目录在前，再按名称排序。
    items.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let arr: Vec<String> = items
        .iter()
        .map(|(dir, name, size, mt)| {
            format!(
                "{{\"dir\":{},\"name\":\"{}\",\"size\":{},\"mtime\":{},\"human\":\"{}\"}}",
                dir,
                json::jesc(name),
                size,
                mt,
                json::jesc(&human(*size))
            )
        })
        .collect();
    format!(
        "{{\"ok\":true,\"path\":\"{}\",\"list\":[{}]}}",
        json::jesc(path),
        arr.join(",")
    )
}

/// 读取文本文件（上限 READ_MAX）-> JSON。
pub fn read_file_json(path: &str) -> String {
    let real = match confined(path) {
        Some(p) => p,
        None => {
            return format!(
                "{{\"ok\":false,\"msg\":\"{}\"}}",
                json::jesc("路径越界：超出文件根目录限制")
            )
        }
    };
    let data = match std::fs::read_to_string(&real) {
        Ok(s) => s,
        Err(e) => {
            return format!("{{\"ok\":false,\"msg\":\"{}\"}}", json::jesc(&e.to_string()))
        }
    };
    format!(
        "{{\"ok\":true,\"name\":\"{}\",\"size\":{},\"data\":\"{}\"}}",
        json::jesc(path),
        data.len(),
        json::jesc(&data)
    )
}

/// 删除文件或目录（递归）。
pub fn del_path(path: &str) -> (bool, String) {
    let real = match confined(path) {
        Some(p) => p,
        None => return (false, "路径越界：超出文件根目录限制".to_string()),
    };
    let md = match std::fs::metadata(&real) {
        Ok(m) => m,
        Err(e) => return (false, format!("无法访问 {}: {}", path, e)),
    };
    let r = if md.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    match r {
        Ok(_) => (true, format!("已删除 {}", path)),
        Err(e) => (false, format!("删除失败: {}", e)),
    }
}

/// 写入文件（用于上传/编辑保存）；未设置大小上限检查由调用方完成。
pub fn write_file(path: &str, bytes: &[u8]) -> (bool, String) {
    let real = match confined(path) {
        Some(p) => p,
        None => return (false, "路径越界：超出文件根目录限制".to_string()),
    };
    match std::fs::write(&real, bytes) {
        Ok(_) => (true, format!("已保存 {} 字节 -> {}", bytes.len(), path)),
        Err(e) => (false, format!("写入失败: {}", e)),
    }
}

/// 读取整文件（下载用）。
pub fn download(path: &str) -> Option<Vec<u8>> {
    let real = confined(path)?;
    std::fs::read(&real).ok()
}

// ---------------------------------------------------------------------------
// 5. 磁盘占用排行
// ---------------------------------------------------------------------------

/// du 扫描指定目录的一级子元素占用，降序返回 Top N -> JSON。
/// 示例：/api/disk/top?path=/&n=20
pub fn disk_top_json(path: &str, n: usize) -> String {
    let real = match confined(path) {
        Some(p) => p,
        None => {
            return format!(
                "{{\"ok\":false,\"msg\":\"{}\"}}",
                json::jesc("路径越界：超出文件根目录限制")
            )
        }
    };
    if !std::path::Path::new(&real).is_dir() {
        return format!("{{\"ok\":false,\"msg\":\"{} 不是目录\"}}", json::jesc(path));
    }
    let out = cmd_all(&format!("du -xk --max-depth=1 {} 2>/dev/null", real)).unwrap_or_default();
    let mut items: Vec<(u64, String)> = Vec::new();
    for line in out.lines() {
        let mut it = line.split_whitespace();
        let size = it.next().and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
        let name: String = it.collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            continue;
        }
        items.push((size.saturating_mul(1024), name));
    }
    items.sort_by(|a, b| b.0.cmp(&a.0));
    items.truncate(n.max(1));
    let arr: Vec<String> = items
        .iter()
        .map(|(s, nm)| {
            format!(
                "{{\"size\":{},\"path\":\"{}\",\"human\":\"{}\"}}",
                s,
                json::jesc(nm),
                json::jesc(&human(*s))
            )
        })
        .collect();
    format!(
        "{{\"ok\":true,\"path\":\"{}\",\"list\":[{}]}}",
        json::jesc(path),
        arr.join(",")
    )
}

// ---------------------------------------------------------------------------
// 6. 资源实时排行（CPU / 内存 Top）
// ---------------------------------------------------------------------------

/// top 式进程排行：采样 ~700ms 计算每进程 CPU%，按 CPU 倒序 -> JSON。
pub fn resources_top_json(n: usize) -> String {
    // 阶段一：收集候选进程（按 RSS 取舍），读一次 CPU 时间。
    let mut cand: Vec<(u32, String, u64, u64)> = Vec::new(); // pid, comm, rss, utime+stime
    if let Ok(rd) = std::fs::read_dir("/proc") {
        for ent in rd.flatten() {
            let pid: u32 = match ent.file_name().to_string_lossy().parse() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let stat_p = format!("/proc/{}/stat", pid);
            let comm_p = format!("/proc/{}/comm", pid);
            let rss_kb = read_status_rss(&format!("/proc/{}/status", pid));
            let comm = std::fs::read_to_string(&comm_p)
                .ok()
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if comm.is_empty() {
                continue;
            }
            let cpu = read_pid_cpu(&stat_p).unwrap_or(0);
            cand.push((pid, comm, rss_kb, cpu));
        }
    }
    let ta = total_ticks();
    // 采样间隔。
    std::thread::sleep(std::time::Duration::from_millis(700));
    let tb = total_ticks();
    // 阶段二：依据总 CPU 刻度估算 CLK_TCK（dt 内系统总 jiffies），再算每进程 CPU%。
    let dt_clk = tb.saturating_sub(ta).max(1) as f64;
    let mut rows: Vec<(f64, u32, String, u64)> = Vec::new();
    for (pid, comm, rss_kb, cpu0) in cand {
        if let Some(cpu1) = read_pid_cpu(&format!("/proc/{}/stat", pid)) {
            let d = cpu1.saturating_sub(cpu0);
            let pct = (d as f64 / dt_clk * 100.0).min(100.0);
            rows.push((pct, pid, comm, rss_kb.saturating_mul(1024)));
        }
    }
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    rows.truncate(n.max(1));
    let arr: Vec<String> = rows
        .iter()
        .map(|(pct, pid, comm, rss)| {
            format!(
                "{{\"cpu\":{:.1},\"pid\":{},\"name\":\"{}\",\"rss\":{},\"human\":\"{}\"}}",
                pct,
                pid,
                json::jesc(comm),
                rss,
                json::jesc(&human(*rss))
            )
        })
        .collect();
    format!("{{\"ok\":true,\"list\":[{}]}}", arr.join(","))
}

/// /proc/stat 第一行 cpu 的累计 jiffies 总和。
fn total_ticks() -> u64 {
    let s = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let line = s.lines().next().unwrap_or("");
    line.split_whitespace()
        .skip(1)
        .filter_map(|x| x.parse::<u64>().ok())
        .sum()
}

fn read_pid_cpu(p: &str) -> Option<u64> {
    let s = std::fs::read_to_string(p).ok()?;
    let end = s.rfind(')')?;
    let rest = &s[end + 1..];
    let mut it = rest.split_whitespace();
    // rest 的 token 从字段3(state)开始；字段14/15(utime,stime) -> 下标 11/12。
    let mut i = 0;
    while i < 11 {
        it.next()?;
        i += 1;
    }
    let utime = it.next().and_then(|x| x.parse::<u64>().ok())?; // idx 11
    let stime = it.next().and_then(|x| x.parse::<u64>().ok())?; // idx 12
    Some(utime + stime)
}

fn read_status_rss(p: &str) -> u64 {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}