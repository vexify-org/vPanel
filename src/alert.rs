//! 资源告警 + 邮件通知：当 CPU/内存/磁盘/带宽超阈值时，通过 SMTP 发邮件。
//!
//! 配置与上次发送时间一起持久化到 `alert.json`（重启也不丢防抖状态）。
//! 由后台低栈线程定期调用 [`check`]，不新增常驻进程表，符合面板极简定位。

use serde::{Deserialize, Serialize};

use crate::json;

const FILE: &str = "alert.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    #[serde(default)]
    pub enabled: bool,
    /// SMTP 服务器地址，如 smtp.example.com
    #[serde(default)]
    pub smtp_host: String,
    /// SMTP 端口。587（STARTTLS）/ 465（SSL）/ 25（明文）。
    #[serde(default = "d_port")]
    pub smtp_port: u16,
    /// 登录账号（留空 = 无需认证）。
    #[serde(default)]
    pub smtp_user: String,
    /// 登录密码。
    #[serde(default)]
    pub smtp_pass: String,
    /// 发件人邮箱。
    #[serde(default)]
    pub from: String,
    /// 收件人邮箱，多个用英文逗号分隔。
    #[serde(default)]
    pub to: String,
    /// 加密模式：starttls | ssl | none。
    #[serde(default = "d_mode")]
    pub mode: String,
    /// CPU 使用率阈值 %（0 = 关闭该项）。
    #[serde(default)]
    pub cpu: f32,
    /// 内存使用率阈值 %（0 = 关闭）。
    #[serde(default)]
    pub mem: f32,
    /// 根分区使用率阈值 %（0 = 关闭）。
    #[serde(default)]
    pub disk: f32,
    /// 下行带宽阈值 B/s（0 = 关闭）。
    #[serde(default)]
    pub net: u64,
    /// 距上次告警的冷却时间（秒），防止告警风暴。
    #[serde(default = "d_cooldown")]
    pub cooldown: u64,
    /// 上次成功发送告警的 unix 时间戳。
    #[serde(default)]
    pub last_sent: u64,
}

fn d_port() -> u16 {
    587
}
fn d_mode() -> String {
    "starttls".into()
}
fn d_cooldown() -> u64 {
    900
}

impl Default for Alert {
    fn default() -> Self {
        Alert {
            enabled: false,
            smtp_host: String::new(),
            smtp_port: d_port(),
            smtp_user: String::new(),
            smtp_pass: String::new(),
            from: String::new(),
            to: String::new(),
            mode: d_mode(),
            cpu: 0.0,
            mem: 0.0,
            disk: 0.0,
            net: 0,
            cooldown: d_cooldown(),
            last_sent: 0,
        }
    }
}

fn alert_path() -> String {
    format!("{}/{}", crate::config::Config::panel_dir(), FILE)
}

fn load() -> Alert {
    std::fs::read_to_string(alert_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(a: &Alert) -> bool {
    serde_json::to_string_pretty(a)
        .map(|j| std::fs::write(alert_path(), j).is_ok())
        .unwrap_or(false)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "vpanel".into())
}

/// 是否支持加密 SMTP（由编译特性决定）。
pub fn tls_ok() -> bool {
    cfg!(feature = "tls")
}

/// 当前各项实时值：cpu %、mem %、disk %、下行带宽 B/s。
pub fn current(mon: &crate::system::Monitor) -> (f32, u32, u32, u64) {
    let cpu = mon.cpu.lock().unwrap().back().copied().unwrap_or(0.0);
    let net = mon.net_dn.lock().unwrap().back().copied().unwrap_or(0);
    let mem = mem_pct();
    let disk = root_disk_pct();
    (cpu, mem, disk, net)
}

fn mem_pct() -> u32 {
    if let Some((total, avail)) = crate::system::mem() {
        if total > 0 {
            ((total - avail) * 100 / total) as u32
        } else {
            0
        }
    } else {
        0
    }
}

/// 根分区（挂载点 "/"）使用率 %。
fn root_disk_pct() -> u32 {
    let out = crate::json::run_out("df", &["-kP"]);
    if let Some(s) = out {
        for line in s.lines().skip(1) {
            let p: Vec<&str> = line.split_whitespace().collect();
            if p.len() >= 6 && p[5] == "/" {
                if let Ok(v) = p[4].replace('%', "").parse::<u32>() {
                    return v;
                }
            }
        }
    }
    0
}

fn fmt_bps(v: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    if v as f64 >= MB {
        format!("{:.1} MB/s", v as f64 / MB)
    } else if v as f64 >= KB {
        format!("{:.1} KB/s", v as f64 / KB)
    } else {
        format!("{} B/s", v)
    }
}

/// 返回基本信息 JSON（前端加载）。
pub fn status_json(mon: &crate::system::Monitor) -> String {
    let a = load();
    let (cpu, mem, disk, net) = current(mon);
    serde_json::json!({
        "ok": true,
        "enabled": a.enabled,
        "tls": tls_ok(),
        "smtp_host": a.smtp_host,
        "smtp_port": a.smtp_port,
        "smtp_user": a.smtp_user,
        "from": a.from,
        "to": a.to,
        "mode": a.mode,
        "cpu": a.cpu,
        "mem": a.mem,
        "disk": a.disk,
        "net": a.net,
        "cooldown": a.cooldown,
        "last_sent": a.last_sent,
        "current": {
            "cpu": cpu,
            "mem": mem,
            "disk": disk,
            "net": net
        }
    })
    .to_string()
}

/// 保存配置（表单）。密码留空则保留旧值，避免每次编辑都要重填。
pub fn save_cfg(
    smtp_host: &str,
    smtp_port: &str,
    smtp_user: &str,
    smtp_pass: &str,
    from: &str,
    to: &str,
    mode: &str,
    cpu: &str,
    mem: &str,
    disk: &str,
    net: &str,
    cooldown: &str,
) -> (bool, String) {
    if smtp_host.trim().is_empty() {
        return (false, "SMTP 服务器地址不能为空".into());
    }
    if from.trim().is_empty() {
        return (false, "发件人邮箱不能为空".into());
    }
    if to.trim().is_empty() {
        return (false, "收件人邮箱不能为空".into());
    }
    let port: u16 = smtp_port.trim().parse().unwrap_or(587);
    let mode = match mode {
        "ssl" | "starttls" | "none" => mode.to_string(),
        _ => d_mode(),
    };
    let mut a = load();
    a.smtp_host = smtp_host.trim().to_string();
    a.smtp_port = port;
    a.smtp_user = smtp_user.trim().to_string();
    if !smtp_pass.is_empty() {
        a.smtp_pass = smtp_pass.to_string(); // 留空保留旧密码
    }
    a.from = from.trim().to_string();
    a.to = to.trim().to_string();
    a.mode = mode.clone();
    a.cpu = cpu.trim().parse::<f32>().unwrap_or(0.0).clamp(0.0, 1000.0);
    a.mem = mem.trim().parse::<f32>().unwrap_or(0.0).clamp(0.0, 1000.0);
    a.disk = disk.trim().parse::<f32>().unwrap_or(0.0).clamp(0.0, 1000.0);
    a.net = net.trim().parse::<u64>().unwrap_or(0);
    a.cooldown = cooldown.trim().parse().unwrap_or(d_cooldown());
    if !save(&a) {
        return (false, "保存失败".into());
    }
    (
        true,
        format!(
            "已保存（{} 加密已{}开启）",
            mode,
            if tls_ok() { "" } else { "未(需 --features tls)" }
        ),
    )
}

/// 启用 / 停用。
pub fn set_enabled(on: bool) -> (bool, String) {
    let mut a = load();
    a.enabled = on;
    save(&a);
    (true, if on { "告警已开启" } else { "告警已停用" }.into())
}

/// 发送测试邮件到配置的收件人，验证 SMTP 是否可用。
pub fn test() -> (bool, String) {
    let a = load();
    if a.smtp_host.is_empty() || a.from.is_empty() || a.to.is_empty() {
        return (false, "请先填写 SMTP 服务器、发件人与收件人".into());
    }
    let recipients = recipients(&a.to);
    let body = format!(
        "您好，\n\n这是一封来自 {} (vPanel) 的测试邮件。\n\n如果收到本邮件，说明 SMTP 配置正确，资源告警将可正常发送。\n\n—— vPanel 告警系统 ({})",
        hostname(),
        now_str()
    );
    match crate::smtp::send(
        &a.smtp_host, a.smtp_port, &a.mode, &a.smtp_user, &a.smtp_pass,
        &a.from, &recipients, "vPanel 告警测试", &body,
    ) {
        Ok(()) => (true, "测试邮件已发送".into()),
        Err(e) => (false, e),
    }
}

fn recipients(to: &str) -> Vec<String> {
    to.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

fn now_str() -> String {
    crate::smtp::format_secs(now())
}

/// 后台检测：超出阈值则发邮件（带冷却防抖）。返回 (是否触发, 提示)。
pub fn check(mon: &crate::system::Monitor) -> (bool, String) {
    let mut a = load();
    if !a.enabled || a.smtp_host.is_empty() {
        return (false, "未开启或未配置".into());
    }
    let (cpu, mem, disk, net) = current(mon);
    let mut items: Vec<String> = Vec::new();
    if a.cpu > 0.0 && cpu >= a.cpu {
        items.push(format!("CPU 使用率 {:.1}% ≥ 阈值 {:.1}%", cpu, a.cpu));
    }
    if a.mem > 0.0 && mem as f32 >= a.mem {
        items.push(format!("内存使用率 {}% ≥ 阈值 {:.1}%", mem, a.mem));
    }
    if a.disk > 0.0 && disk as f32 >= a.disk {
        items.push(format!("根分区使用率 {}% ≥ 阈值 {:.1}%", disk, a.disk));
    }
    if a.net > 0 && net >= a.net {
        items.push(format!("下行带宽 {} ≥ 阈值 {}", fmt_bps(net), fmt_bps(a.net)));
    }
    if items.is_empty() {
        return (false, "各项指标均正常".into());
    }
    // 冷却：距上次发送不足则跳过。
    let now = now();
    if now - a.last_sent < a.cooldown {
        return (false, "冷却中，暂不发送".into());
    }
    let body = format!(
        "服务器 {} 资源告警：\n\n{}\n\n当前实时值：CPU {:.1}% · 内存 {}% · 磁盘 {}% · 下行 {}。\n\n触发时间：{}\n—— vPanel 告警系统",
        hostname(),
        items.join("\n"),
        cpu,
        mem,
        disk,
        fmt_bps(net),
        now_str()
    );
    let subject = format!("[vPanel 告警] {} 资源异常", hostname());
    let recipients = recipients(&a.to);
    match crate::smtp::send(
        &a.smtp_host, a.smtp_port, &a.mode, &a.smtp_user, &a.smtp_pass,
        &a.from, &recipients, &subject, &body,
    ) {
        Ok(()) => {
            a.last_sent = now;
            save(&a);
            (true, "已发送告警邮件".into())
        }
        Err(e) => (false, e),
    }
}