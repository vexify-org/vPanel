//! 自研防火墙：独立链 + nft/iptables 双后端，规则持久化到 JSON。
//!
//! 不再依赖 ufw。规则写在独立链/独立表里（避免干扰系统默认链），
//! 操作类需要 root 权限；无 root 或缺少后端时返回明确的错误文本。

use serde::{Deserialize, Serialize};

use crate::json;

const FILE: &str = "firewall.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Rule {
    #[serde(default)]
    id: u64,
    /// allow 放行 / deny 拒绝。
    #[serde(default = "d_allow")]
    action: String,
    /// "" 表示所有端口；或 8080；或范围 8080-9090。
    #[serde(default)]
    port: String,
    /// tcp | udp | both。
    #[serde(default = "d_tcp")]
    proto: String,
    /// "" 表示任意来源；或 1.2.3.4 / 1.2.3.0/24。
    #[serde(default)]
    ip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FwState {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    seq: u64,
    #[serde(default)]
    rules: Vec<Rule>,
}

fn d_allow() -> String {
    "allow".into()
}
fn d_tcp() -> String {
    "tcp".into()
}

impl Default for FwState {
    fn default() -> Self {
        FwState { enabled: false, seq: 0, rules: Vec::new() }
    }
}

fn fw_path() -> String {
    format!("{}/{}", crate::config::Config::panel_dir(), FILE)
}

fn load() -> FwState {
    std::fs::read_to_string(fw_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(st: &FwState) -> bool {
    serde_json::to_string_pretty(st)
        .map(|j| std::fs::write(fw_path(), j).is_ok())
        .unwrap_or(false)
}

fn avail(bin: &str) -> bool {
    std::process::Command::new(bin)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 检测可用后端：nft → iptables。
pub fn backend() -> String {
    if avail("nft") {
        return "nft".into();
    }
    if avail("iptables") {
        return "iptables".into();
    }
    "none".into()
}

fn run_cmd(cmd: &str, args: &[&str]) -> bool {
    std::process::Command::new(cmd)
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn verdict(action: &str) -> &'static str {
    if action == "deny" {
        "drop"
    } else {
        "accept"
    }
}

fn protos(p: &str, out: &mut Vec<&'static str>) {
    match p {
        "udp" => out.push("udp"),
        "both" => {
            out.push("tcp");
            out.push("udp");
        }
        _ => out.push("tcp"),
    }
}

/// 渲染 nftables ruleset（独立 inet 表 vpanel，hook input，policy accept）。
fn render_nft(st: &FwState) -> String {
    let mut s = String::new();
    s.push_str("table inet vpanel {\n");
    s.push_str("  chain input {\n");
    s.push_str("    type filter hook input priority filter; policy accept;\n");
    s.push_str("    iif \"lo\" accept\n");
    s.push_str("    ct state established,related accept\n");
    for r in &st.rules {
        let mut ps = Vec::new();
        protos(&r.proto, &mut ps);
        let saddr = if r.ip.is_empty() {
            String::new()
        } else {
            format!("ip saddr {} ", r.ip)
        };
        for p in ps {
            let dport = if r.port.is_empty() {
                String::new()
            } else {
                format!("{} dport {} ", p, r.port)
            };
            s.push_str(&format!("    {}{}{}\n", saddr, dport, verdict(&r.action)));
        }
    }
    s.push_str("  }\n}\n");
    s
}

fn apply_nft(st: &FwState) -> Result<String, String> {
    // 无论启用与否，先清空既有 vpanel 表（幂等）。
    let _ = run_cmd("nft", &["delete", "table", "inet", "vpanel"]);
    if !st.enabled || st.rules.is_empty() {
        return Ok(format!("已{}（nft）", if st.enabled { "启用但无规则" } else { "停用" }));
    }
    let script = render_nft(st);
    let tf = format!("{}/.fw_nft.rules", crate::config::Config::panel_dir());
    if std::fs::write(&tf, &script).is_err() {
        return Err("写临时规则文件失败".into());
    }
    let out = std::process::Command::new("nft").args(["-f", &tf]).output();
    let _ = std::fs::remove_file(&tf);
    match out {
        Ok(o) if o.status.success() => Ok(format!("已应用 {} 条规则（nft）", st.rules.len())),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

fn apply_iptables(st: &FwState) -> Result<String, String> {
    // 摘掉既有 vpanel 链及其 INPUT 跳转，幂等重建。
    let _ = run_cmd("iptables", &["-D", "INPUT", "-j", "vpanel"]);
    let _ = run_cmd("iptables", &["-F", "vpanel"]);
    let _ = run_cmd("iptables", &["-X", "vpanel"]);
    if !st.enabled || st.rules.is_empty() {
        return Ok("已停用（iptables）".into());
    }
    if !run_cmd("iptables", &["-N", "vpanel"]) {
        return Err("创建 vpanel 链失败".into());
    }
    let mut ok = true;
    for r in &st.rules {
        let mut ps = Vec::new();
        protos(&r.proto, &mut ps);
        for p in ps {
            let v = if r.action == "deny" { "DROP" } else { "ACCEPT" };
            let mut a: Vec<String> = vec!["-A".into(), "vpanel".into()];
            if !r.ip.is_empty() {
                a.push("-s".into());
                a.push(r.ip.clone());
            }
            a.push("-p".into());
            a.push(p.to_string());
            if !r.port.is_empty() {
                a.push("--dport".into());
                a.push(r.port.clone());
            }
            a.push("-j".into());
            a.push(v.into());
            let args: Vec<&str> = a.iter().map(String::as_str).collect();
            if !run_cmd("iptables", &args) {
                ok = false;
            }
        }
    }
    run_cmd("iptables", &["-I", "INPUT", "1", "-j", "vpanel"]);
    if ok {
        Ok(format!("已应用 {} 条规则（iptables）", st.rules.len()))
    } else {
        Err("部分 iptables 规则应用失败".into())
    }
}

/// 把当前状态同步到系统防火墙。停用或空规则时清除独立链/表。
fn apply(st: &FwState) -> Result<String, String> {
    match backend().as_str() {
        "nft" => apply_nft(st),
        "iptables" => apply_iptables(st),
        _ => Err("未检测到 nft 或 iptables，规则未应用".into()),
    }
}

fn valid_port(p: &str) -> bool {
    if p.is_empty() {
        return true;
    }
    // 允许：单端口 "8080"；范围 "8080-9090"。其余为不合法写法。
    if p.starts_with('-') || p.ends_with('-') || p.contains("--") {
        return false;
    }
    let segs: Vec<&str> = p.split('-').collect();
    if segs.is_empty() || segs.len() > 2 {
        return false;
    }
    segs.iter()
        .all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
}

fn valid_ip(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    !s.chars().any(|c| {
        !c.is_ascii_alphanumeric() && !matches!(c, '.' | ':' | '/' | '-' | '_')
    })
}

pub fn rules_json() -> String {
    let st = load();
    let mut list = Vec::with_capacity(st.rules.len());
    for r in &st.rules {
        list.push(format!(
            "{{\"id\":{},\"action\":\"{}\",\"port\":\"{}\",\"proto\":\"{}\",\"ip\":\"{}\"}}",
            r.id,
            json::jesc(&r.action),
            json::jesc(&r.port),
            json::jesc(&r.proto),
            json::jesc(&r.ip)
        ));
    }
    format!(
        "{{\"ok\":true,\"enabled\":{},\"backend\":\"{}\",\"len\":{},\"list\":[{}]}}",
        st.enabled,
        json::jesc(&backend()),
        list.len(),
        list.join(",")
    )
}

pub fn status_json() -> String {
    let st = load();
    format!(
        "{{\"ok\":true,\"enabled\":{},\"backend\":\"{}\",\"count\":{}}}",
        st.enabled,
        json::jesc(&backend()),
        st.rules.len()
    )
}

/// 添加规则。action=allow|deny，proto=tcp|udp|both，port/ip 可为空。
pub fn add(action: &str, port: &str, proto: &str, ip: &str) -> (bool, String) {
    let action = if action == "deny" { "deny".to_string() } else { "allow".to_string() };
    let proto = match proto {
        "udp" => "udp".to_string(),
        "both" => "both".to_string(),
        _ => "tcp".to_string(),
    };
    let port = port.trim().to_string();
    let ip = ip.trim().to_string();
    if !valid_port(&port) {
        return (false, "端口不合法（允许空、单端口或 n-m 范围）".into());
    }
    if !valid_ip(&ip) {
        return (false, "来源 IP/网段不合法".into());
    }
    let mut st = load();
    st.seq += 1;
    st.rules.push(Rule { id: st.seq, action, port, proto, ip });
    save(&st);
    match apply(&st) {
        Ok(m) => (true, m),
        Err(e) => (false, e),
    }
}

/// 按 id 删除规则。
pub fn del(id_str: &str) -> (bool, String) {
    let id: u64 = match id_str.trim().parse() {
        Ok(v) => v,
        Err(_) => return (false, "id 需为数字".into()),
    };
    let mut st = load();
    let before = st.rules.len();
    st.rules.retain(|r| r.id != id);
    if st.rules.len() == before {
        return (false, "未找到该规则".into());
    }
    save(&st);
    match apply(&st) {
        Ok(m) => (true, m),
        Err(e) => (false, e),
    }
}

/// 按端口删除（MCP / 旧接口兼容）。
pub fn del_by_port(port: &str) -> (bool, String) {
    let p = port.trim().to_string();
    let mut st = load();
    let before = st.rules.len();
    st.rules.retain(|r| r.port == p);
    if st.rules.len() == before {
        return (false, "未找到该端口规则".into());
    }
    save(&st);
    match apply(&st) {
        Ok(m) => (true, m),
        Err(e) => (false, e),
    }
}

/// 启用 / 停用防火墙。停用时清除独立链/表。
pub fn set_enabled(on: bool) -> (bool, String) {
    let mut st = load();
    st.enabled = on;
    save(&st);
    match apply(&st) {
        Ok(m) => (true, m),
        Err(e) => (false, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_port_accepts_single_and_range() {
        assert!(valid_port(""));
        assert!(valid_port("8080"));
        assert!(valid_port("53"));
        assert!(valid_port("8000-9000"));
    }

    #[test]
    fn valid_port_rejects_bad() {
        assert!(!valid_port("-1"));
        assert!(!valid_port("80-"));
        assert!(!valid_port("80--90"));
        assert!(!valid_port("a80"));
        assert!(!valid_port("80-90-100"));
        assert!(!valid_port(" "));
    }

    #[test]
    fn valid_ip_accepts_common_forms() {
        assert!(valid_ip(""));
        assert!(valid_ip("1.2.3.4"));
        assert!(valid_ip("2001:db8::1"));
        assert!(valid_ip("10.0.0.1/24"));
        assert!(valid_ip("a-b_c.d"));
    }

    #[test]
    fn valid_ip_rejects_spaces_and_special() {
        assert!(!valid_ip("1.2.3.4 5"));
        assert!(!valid_ip("a@b"));
        assert!(!valid_ip("a;b"));
        assert!(!valid_ip("a(b)"));
    }

    #[test]
    fn verdict_maps_action_words() {
        assert_eq!(verdict("allow"), "accept");
        assert_eq!(verdict("deny"), "drop");
        // 未知动作默认 accept
        assert_eq!(verdict("something-else"), "accept");
    }
}