//! 内置工具 · 网络 / 地址 / 端口（`ops8`）。
//! 纯函数计算 IP / CIDR / 端口 / 域名等，不依赖外部 crate，不发起网络请求。

use crate::json;

fn j(s: &str) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(s))
}
fn jn(n: String) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(&n))
}
fn iv(t: &str) -> u64 {
    t.trim().parse::<u64>().unwrap_or(0)
}
fn i4(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.trim().split('.').collect();
    if parts.len() != 4 { return None; }
    let mut v: u32 = 0;
    for p in parts {
        let b: u32 = p.parse::<u32>().ok()?;
        if b > 255 { return None; }
        v = (v << 8) | b;
    }
    Some(v)
}
fn f4(n: u32) -> String {
    format!("{}.{}.{}.{}", n >> 24, (n >> 16) & 255, (n >> 8) & 255, n & 255)
}
fn prefix(p: &str) -> u32 {
    let v: u32 = p.trim().parse().unwrap_or(0);
    v.min(32)
}

// ---- IPv4 基础 ----
pub fn ip_valid_v4(t: &str) -> String { j(if i4(t).is_some() { "true" } else { "false" }) }
pub fn ip_octets(t: &str) -> String {
    let parts: Vec<&str> = t.trim().split('.').collect();
    if parts.len() != 4 { return j("无效IPv4"); }
    format!("{{\"result\":[{},{},{},{}]}}", parts[0], parts[1], parts[2], parts[3])
}
pub fn ip_octet_at(t: &str, i: &str) -> String {
    let idx = iv(i) as usize;
    let v: [u8; 4] = match i4(t) { Some(n) => [(n >> 24) as u8, ((n >> 16) & 255) as u8, ((n >> 8) & 255) as u8, (n & 255) as u8], None => return j("无效IPv4") };
    match v.get(idx) { Some(&x) => jn(x.to_string()), None => j("索引0-3") }
}
pub fn ip_to_int(t: &str) -> String { match i4(t) { Some(n) => jn(n.to_string()), None => j("无效IPv4") } }
pub fn ip_from_int(t: &str) -> String {
    if iv(t) > 4294967295 { return j("超出32位"); }
    j(&f4(iv(t) as u32))
}
pub fn ip_increment(t: &str) -> String { match i4(t) { Some(n) if n < 4294967295 => j(&f4(n + 1)), _ => j("已是最大值") } }
pub fn ip_decrement(t: &str) -> String { match i4(t) { Some(n) if n > 0 => j(&f4(n - 1)), _ => j("已是最小值") } }
pub fn ip_is_private(t: &str) -> String {
    match i4(t) {
        Some(n) => {
            let private = (n >> 24 == 10) || ((n & 0xFFF00000) == 0xAC100000) || ((n & 0xFFFF0000) == 0xC0A80000);
            j(if private { "true" } else { "false" })
        }
        None => j("无效IPv4"),
    }
}
pub fn ip_is_loopback(t: &str) -> String { match i4(t) { Some(n) => j(if n >> 24 == 127 { "true" } else { "false" }), None => j("无效IPv4") } }
pub fn ip_is_multicast(t: &str) -> String { match i4(t) { Some(n) => j(if (224..=239).contains(&(n >> 24)) { "true" } else { "false" }), None => j("无效IPv4") } }
pub fn ip_is_link_local(t: &str) -> String { match i4(t) { Some(n) => j(if n >> 16 == 0xA9FE { "true" } else { "false" }), None => j("无效IPv4") } }
pub fn ip_class(t: &str) -> String {
    match i4(t).map(|n| n >> 24) {
        Some(a) => {
            let c = if a <= 126 { "A" } else if a == 127 { "回环" } else if a <= 191 { "B" } else if a <= 223 { "C" } else if a <= 239 { "D组播" } else { "E保留" };
            j(c)
        }
        None => j("无效IPv4"),
    }
}
pub fn ip_reverse(t: &str) -> String {
    match i4(t) { Some(n) => j(&f4((n & 0xFF) << 24 | ((n >> 8) & 0xFF) << 16 | ((n >> 16) & 0xFF) << 8 | (n >> 24))), None => j("无效IPv4") }
}

// ---- CIDR / 子网 ----
pub fn mask_from_prefix(t: &str) -> String {
    let p = prefix(t);
    let m = if p == 0 { 0 } else { u32::MAX << (32 - p) };
    j(&f4(m))
}
pub fn prefix_from_mask(t: &str) -> String {
    match i4(t) {
        Some(m) => {
            let mut ones = 0u32;
            let mut cont = true;
            for bit in (0..32).rev() {
                if (m >> bit) & 1 == 1 { if !cont { return j("非连续掩码"); } ones += 1; } else { cont = false; }
            }
            jn(ones.to_string())
        }
        None => j("无效掩码"),
    }
}
pub fn subnet_network(t: &str, a: &str) -> String {
    match i4(t) { Some(n) => { let m = if prefix(a) == 0 { 0 } else { u32::MAX << (32 - prefix(a)) }; j(&f4(n & m)) }, None => j("无效IPv4") }
}
pub fn subnet_broadcast(t: &str, a: &str) -> String {
    match i4(t) { Some(n) => { let p = prefix(a); let inv = if p == 0 { u32::MAX } else { !(u32::MAX << (32 - p)) }; j(&f4(n | inv)) }, None => j("无效IPv4") }
}
pub fn subnet_hosts(t: &str, a: &str) -> String {
    let p = prefix(a);
    if p == 32 { return jn("0".to_string()); }
    if p >= 31 { return jn((2u64.pow(32 - p)).to_string()); }
    jn((2u64.pow(32 - p) - 2).to_string())
}
pub fn cidr_contains(t: &str, a: &str) -> String {
    // t = "ip/prefix", a = 目标 ip
    let (addr, pfx) = match t.split_once('/') {
        Some((x, y)) => (x, y.trim().parse::<u32>().unwrap_or(0).min(32)),
        None => return j("需要 cidr 格式 ip/prefix"),
    };
    match (i4(addr), i4(a)) {
        (Some(n), Some(m2)) => {
            let mask = if pfx == 0 { 0 } else { u32::MAX << (32 - pfx) };
            j(if (n & mask) == (m2 & mask) { "true" } else { "false" })
        }
        _ => j("无效IP"),
    }
}
pub fn cidr_first_ip(t: &str) -> String {
    let (addr, n) = match t.split_once('/') { Some(x) => x, None => return j("需要 ip/prefix") };
    match i4(addr) { Some(a) => { let p = n.trim().parse::<u32>().unwrap_or(0).min(32); let m = if p == 0 { 0 } else { u32::MAX << (32 - p) }; j(&f4(a & m)) }, None => j("无效IPv4") }
}
pub fn cidr_last_ip(t: &str) -> String {
    let (addr, n) = match t.split_once('/') { Some(x) => x, None => return j("需要 ip/prefix") };
    match i4(addr) { Some(a) => { let p = n.trim().parse::<u32>().unwrap_or(0).min(32); let m = if p == 0 { 0 } else { u32::MAX << (32 - p) }; let inv = if p == 0 { u32::MAX } else { !(u32::MAX << (32 - p)) }; j(&f4(a | m | inv)) }, None => j("无效IPv4") }
}
pub fn cidr_size(t: &str) -> String {
    let n = match t.split_once('/') { Some(x) => x.1.trim().parse::<u32>().unwrap_or(0).min(32), None => return j("需要 ip/prefix") };
    jn(2u64.pow(32 - n).to_string())
}

// ---- IPv6 / MAC ----
pub fn ipv6_valid(t: &str) -> String {
    let s = t.trim();
    if s.is_empty() || s.len() < 2 { return j("false"); }
    if s.contains(":::") { return j("false"); }
    let groups: Vec<&str> = s.split("::").collect();
    if groups.len() > 2 { return j("false"); }
    let mut total = 0usize;
    for g in &groups {
        for seg in g.split(':') {
            if seg.is_empty() { continue; }
            if !seg.chars().all(|c| c.is_ascii_hexdigit()) || seg.len() > 4 { return j("false"); }
            total += 1;
        }
    }
    if groups.len() == 2 && total < 1 { return j("false"); }
    j("true")
}
pub fn ipv6_groups(t: &str) -> String {
    let s = t.trim();
    let mut total = 0usize;
    for g in s.split("::") {
        for seg in g.split(':') { if !seg.is_empty() { total += 1; } }
    }
    jn(total.to_string())
}
pub fn mac_valid(t: &str) -> String {
    let c = t.trim().replace(':', "").replace('-', "");
    j(if c.len() == 12 && c.chars().all(|x| x.is_ascii_hexdigit()) { "true" } else { "false" })
}
pub fn mac_colon(t: &str) -> String {
    let c = t.trim().replace(':', "").replace('-', "");
    if c.len() != 12 || !c.chars().all(|x| x.is_ascii_hexdigit()) { return j("无效MAC"); }
    let mut out = Vec::new();
    for i in (0..12).step_by(2) { out.push(&c[i..i + 2]); }
    j(&out.join(":"))
}
pub fn mac_is_unicast(t: &str) -> String {
    let c = t.trim().replace(':', "").replace('-', "");
    if c.len() != 12 { return j("无效MAC"); }
    let first = u8::from_str_radix(&c[0..2], 16).unwrap_or(0);
    j(if first & 1 == 0 { "true" } else { "false" })
}
pub fn mac_is_multicast(t: &str) -> String {
    let c = t.trim().replace(':', "").replace('-', "");
    if c.len() != 12 { return j("无效MAC"); }
    let first = u8::from_str_radix(&c[0..2], 16).unwrap_or(0);
    j(if first & 1 == 1 { "true" } else { "false" })
}

// ---- 端口 ----
pub fn port_valid(t: &str) -> String {
    let p = iv(t);
    j(if p <= 65535 { "true" } else { "false" })
}
fn port_name(p: u64) -> &'static str {
    match p {
        80 => "HTTP", 443 => "HTTPS", 22 => "SSH", 21 => "FTP", 25 => "SMTP", 110 => "POP3",
        143 => "IMAP", 3306 => "MySQL", 5432 => "PostgreSQL", 6379 => "Redis", 27017 => "MongoDB",
        53 => "DNS", 67 => "DHCP", 123 => "NTP", 161 => "SNMP", 993 => "IMAPS", 995 => "POP3S",
        3389 => "RDP", 5900 => "VNC", 8080 => "HTTP-Alt", 8443 => "HTTPS-Alt", 8000 => "HTTP-Dev",
        9000 => "PHP-FPM/Dev", 2375 => "Docker", 11211 => "Memcached", 1883 => "MQTT", 9200 => "Elasticsearch",
        _ => "未知",
    }
}
pub fn port_service(t: &str) -> String { j(port_name(iv(t))) }
pub fn port_class(t: &str) -> String {
    let p = iv(t);
    j(if p <= 1023 { "知名端口(0-1023)" } else if p <= 49151 { "注册端口(1024-49151)" } else { "动态端口(49152-65535)" })
}
pub fn port_range_count(t: &str) -> String {
    let (a, b) = match t.trim().split_once('-') { Some(x) => (x.0.trim().parse::<u64>().unwrap_or(0), x.1.trim().parse::<u64>().unwrap_or(0)), None => return j("需要 n-m 格式") };
    if a > b { return j("起始需<=结束"); }
    jn((b - a + 1).to_string())
}
pub fn port_in_range(t: &str, a: &str) -> String {
    let (lo, hi) = match t.trim().split_once('-') { Some(x) => (x.0.trim().parse::<u64>().unwrap_or(0), x.1.trim().parse::<u64>().unwrap_or(0)), None => return j("需要 n-m 格式") };
    let p = iv(a);
    j(if p >= lo && p <= hi { "true" } else { "false" })
}

// ---- 域名 / 邮箱 ----
pub fn domain_valid(t: &str) -> String {
    let s = t.trim();
    let ok = !s.is_empty() && s.len() <= 253 && !s.contains(' ') && s.split('.').all(|l| {
        !l.is_empty() && l.len() <= 63 && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !l.starts_with('-') && !l.ends_with('-')
    });
    j(if ok { "true" } else { "false" })
}
pub fn domain_labels(t: &str) -> String {
    let c = t.trim().split('.').filter(|l| !l.is_empty()).count();
    jn(c.to_string())
}
pub fn domain_tld(t: &str) -> String {
    j(&t.trim().split('.').filter(|l| !l.is_empty()).last().unwrap_or_default().to_uppercase())
}
pub fn domain_has_www(t: &str) -> String { j(if t.trim().to_lowercase().starts_with("www.") { "true" } else { "false" }) }
pub fn email_valid(t: &str) -> String {
    let s = t.trim();
    if let Some((local, dom)) = s.split_once('@') {
        let ok = !local.is_empty() && dom.contains('.') && !local.contains(' ') && !local.contains('@');
        j(if ok { "true" } else { "false" })
    } else { j("false") }
}
pub fn email_local(t: &str) -> String { j(&t.trim().split('@').next().unwrap_or_default()) }
pub fn email_domain(t: &str) -> String { j(&t.trim().split('@').nth(1).unwrap_or_default()) }

// ---- HTTP 状态 ----
fn http_name(code: u64) -> &'static str {
    match code {
        200 => "OK", 201 => "Created", 204 => "No Content", 301 => "Moved Permanently", 302 => "Found",
        304 => "Not Modified", 400 => "Bad Request", 401 => "Unauthorized", 403 => "Forbidden",
        404 => "Not Found", 405 => "Method Not Allowed", 409 => "Conflict", 413 => "Payload Too Large",
        429 => "Too Many Requests", 500 => "Internal Server Error", 501 => "Not Implemented",
        502 => "Bad Gateway", 503 => "Service Unavailable", 504 => "Gateway Timeout",
        _ => "未知",
    }
}
pub fn http_status_name(t: &str) -> String { j(http_name(iv(t))) }
pub fn http_status_class(t: &str) -> String {
    let c = iv(t) / 100;
    let name = match c { 1 => "信息", 2 => "成功", 3 => "重定向", 4 => "客户端错误", 5 => "服务端错误", _ => "非法状态码" };
    j(name)
}
pub fn is_2xx(t: &str) -> String { let c = iv(t); j(if (200..300).contains(&c) { "true" } else { "false" }) }
pub fn is_4xx(t: &str) -> String { let c = iv(t); j(if (400..500).contains(&c) { "true" } else { "false" }) }
pub fn is_5xx(t: &str) -> String { let c = iv(t); j(if (500..600).contains(&c) { "true" } else { "false" }) }

// ---- IP 比较 ----
pub fn ip_compare(t: &str, a: &str) -> String {
    match (i4(t), i4(a)) { (Some(x), Some(y)) => jn((x.cmp(&y) as i8).to_string()), _ => j("无效IP") }
}
pub fn ip_is_broadcast(t: &str) -> String {
    match i4(t) { Some(n) => j(if (n & 0xFF) == 255 { "true" } else { "false" }), None => j("无效IPv4") }
}
pub fn ip_is_zero(t: &str) -> String { match i4(t) { Some(n) => j(if n == 0 { "true" } else { "false" }), None => j("无效IPv4") } }
pub fn ip_wildcard_mask(t: &str) -> String {
    match i4(t) { Some(n) => j(&f4(!n)), None => j("无效IPv4") }
}