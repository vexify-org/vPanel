//! 内置工具 · 安全 / 颜色 / 数论 / 运维 / 日志（`ops10`）。
//! 全部为纯函数，无状态、不依赖外部 crate。

use crate::json;

fn j(s: &str) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(s))
}
fn jn(n: String) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(&n))
}
fn iv(t: &str) -> i64 {
    t.trim().parse::<i64>().unwrap_or(0)
}
fn uv(t: &str) -> u64 {
    t.trim().parse::<u64>().unwrap_or(0)
}

// ===================== 熵 / 密码强度 =====================
pub fn entropy_of(t: &str) -> String {
    let total = t.chars().count();
    if total == 0 { return jn("0.0".to_string()); }
    let mut fq: std::collections::HashMap<char, usize> = std::collections::HashMap::new();
    for c in t.chars() { *fq.entry(c).or_insert(0) += 1; }
    let mut e = 0.0f64;
    for (_, cnt) in fq {
        let p = cnt as f64 / total as f64;
        e -= p * p.log2();
    }
    jn(format!("{:.4}", e))
}
pub fn sec_charset_size(t: &str) -> String {
    let set: std::collections::HashSet<char> = t.chars().collect();
    jn(set.len().to_string())
}
pub fn pass_class_count(t: &str) -> String {
    let mut n = 0;
    if t.chars().any(|c| c.is_ascii_uppercase()) { n += 1; }
    if t.chars().any(|c| c.is_ascii_lowercase()) { n += 1; }
    if t.chars().any(|c| c.is_ascii_digit()) { n += 1; }
    if t.chars().any(|c| !c.is_ascii_alphanumeric()) { n += 1; }
    jn(n.to_string())
}
pub fn pass_has_upper(t: &str) -> String { j(if t.chars().any(|c| c.is_ascii_uppercase()) { "true" } else { "false" }) }
pub fn pass_has_lower(t: &str) -> String { j(if t.chars().any(|c| c.is_ascii_lowercase()) { "true" } else { "false" }) }
pub fn pass_has_digit(t: &str) -> String { j(if t.chars().any(|c| c.is_ascii_digit()) { "true" } else { "false" }) }
pub fn pass_has_special(t: &str) -> String { j(if t.chars().any(|c| !c.is_ascii_alphanumeric()) { "true" } else { "false" }) }
pub fn pass_strength(t: &str) -> String {
    let len = t.chars().count();
    let classes = pass_class_count(t).trim().trim_matches('"').parse::<i64>().unwrap_or(0);
    let e = entropy_of(t).trim().trim_matches('"').parse::<f64>().unwrap_or(0.0);
    let mut score = 0i64;
    if len >= 8 { score += 1; }
    if len >= 12 { score += 1; }
    score += classes.min(4);
    if e >= 3.0 { score += 1; }
    if score > 6 { score = 6; }
    let d = match score {
        6 => "极强", 5 => "很强", 4 => "较强", 3 => "中等", 2 => "较弱", _ => "很弱",
    };
    j(d)
}
pub fn pass_estimate_bits(t: &str) -> String {
    let len = t.chars().count() as f64;
    let charset = sec_charset_size(t).trim().trim_matches('"').parse::<f64>().unwrap_or(0.0);
    if len == 0.0 || charset == 0.0 { return jn("0".to_string()); }
    let bits = len * (charset as f64).log2();
    jn(format!("{:.1}", bits))
}

// ===================== HTML / URI =====================
const HTML_ENT: [(&str, &str); 5] = [("&amp;", "&"), ("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&#39;", "'")];
pub fn html_escape(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    j(&out)
}
pub fn html_unescape(t: &str) -> String {
    let mut out = t.to_string();
    for (from, to) in HTML_ENT.iter() {
        out = out.replace(from, to);
    }
    j(&out)
}
pub fn uri_scheme(t: &str) -> String {
    match t.split_once("://") {
        Some((s, _)) => j(s),
        None => j(""),
    }
}
pub fn uri_segments(t: &str) -> String {
    let path = match t.find("://") {
        Some(i) => &t[i + 3..],
        None => t,
    };
    let path = match path.find('/') { Some(i) => &path[i..], None => "" };
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let arr: Vec<String> = segs.iter().map(|&s| format!("\"{}\"", json::jesc(s))).collect();
    format!("{{\"result\":[{}]}}", arr.join(","))
}

// ===================== 颜色 =====================
fn hex_tuple(h: &str) -> Option<(u32, u32, u32)> {
    let h = h.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) { return None; }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(((v >> 16) & 255, (v >> 8) & 255, v & 255))
}
pub fn hex_to_rgb(t: &str) -> String {
    match hex_tuple(t) {
        Some((r, g, b)) => format!("{{\"result\":[{},{},{}]}}", r, g, b),
        None => j("无效十六进制颜色"),
    }
}
pub fn rgb_to_hex(t: &str) -> String {
    let p: Vec<u32> = t.split(',').map(|s| s.trim().parse::<u32>().unwrap_or(0).min(255)).collect();
    if p.len() != 3 { return j("需要 r,g,b"); }
    j(&format!("#{:02X}{:02X}{:02X}", p[0], p[1], p[2]))
}
pub fn is_hex(t: &str) -> String {
    j(if hex_tuple(t).is_some() { "true" } else { "false" })
}
pub fn hex_brightness(t: &str) -> String {
    match hex_tuple(t) {
        Some((r, g, b)) => {
            // 感知亮度 (0-255)
            let l = (0.29900 * r as f64 + 0.58700 * g as f64 + 0.11400 * b as f64).round();
            jn(l.to_string())
        }
        None => j("无效颜色"),
    }
}
pub fn hex_is_light(t: &str) -> String {
    let b = hex_brightness(t).trim().trim_matches('"').parse::<f64>().unwrap_or(0.0);
    j(if b >= 128.0 { "true" } else { "false" })
}
pub fn hex_complement(t: &str) -> String {
    match hex_tuple(t) {
        Some((r, g, b)) => j(&format!("#{:02X}{:02X}{:02X}", 255 - r, 255 - g, 255 - b)),
        None => j("无效颜色"),
    }
}
pub fn hex_lighten(t: &str, a: &str) -> String {
    let amt: f64 = a.trim().parse().unwrap_or(0.0);
    match hex_tuple(t) {
        Some((r, g, b)) => {
            let f = |x: u32| ((x as f64 + (255.0 - x as f64) * amt.max(0.0).min(1.0)).round() as u32).min(255);
            j(&format!("#{:02X}{:02X}{:02X}", f(r), f(g), f(b)))
        }
        None => j("无效颜色"),
    }
}
pub fn hex_darken(t: &str, a: &str) -> String {
    let amt: f64 = a.trim().parse().unwrap_or(0.0);
    match hex_tuple(t) {
        Some((r, g, b)) => {
            let f = |x: u32| ((x as f64 * (1.0 - amt.max(0.0).min(1.0))).round() as u32).min(255);
            j(&format!("#{:02X}{:02X}{:02X}", f(r), f(g), f(b)))
        }
        None => j("无效颜色"),
    }
}
pub fn hex_contrast(t: &str, a: &str) -> String {
    // WCAG 对比度
    let f = |c: &str| -> Option<f64> {
        let t = hex_tuple(c)?;
        let l = |x: u32| {
            let v = x as f64 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        let lum = 0.2126 * l(t.0) + 0.7152 * l(t.1) + 0.0722 * l(t.2);
        Some(lum)
    };
    match (f(t), f(a)) {
        (Some(a), Some(b)) => {
            let (hi, lo) = if a > b { (a, b) } else { (b, a) };
            jn(format!("{:.2}", (hi + 0.05) / (lo + 0.05)))
        }
        _ => j("无效颜色"),
    }
}
pub fn hex_blend(t: &str, a: &str, r: &str) -> String {
    let ratio: f64 = r.trim().parse::<f64>().unwrap_or(0.5).clamp(0.0, 1.0);
    match (hex_tuple(t), hex_tuple(a)) {
        (Some((r1, g1, b1)), Some((r2, g2, b2))) => {
            let f = |x: u32, y: u32| ((x as f64 * (1.0 - ratio) + y as f64 * ratio).round() as u32);
            j(&format!("#{:02X}{:02X}{:02X}", f(r1, r2), f(g1, g2), f(b1, b2)))
        }
        _ => j("无效颜色"),
    }
}

// ===================== 数论 =====================
pub fn math_fact(t: &str) -> String {
    let n = iv(t);
    if n < 0 { return jn("0".to_string()); }
    if n > 20 { return j("数字过大(>20)"); }
    let mut r: u64 = 1;
    for i in 1..=n as u64 { r *= i; }
    jn(r.to_string())
}
pub fn math_gcd(t: &str, a: &str) -> String {
    let (mut a1, mut b1) = (uv(t), uv(a));
    while b1 != 0 { let m = a1 % b1; a1 = b1; b1 = m; }
    jn(a1.to_string())
}
pub fn math_lcm(t: &str, a: &str) -> String {
    let (x, y) = (uv(t), uv(a));
    if x == 0 || y == 0 { return jn("0".to_string()); }
    let g = { let (mut a1, mut b1) = (x, y); while b1 != 0 { let m = a1 % b1; a1 = b1; b1 = m; } a1 };
    jn((x / g * y).to_string())
}
pub fn math_modpow(t: &str, a: &str, m: &str) -> String {
    let mut base = uv(t) % uv(m).max(1);
    let mut exp = uv(a);
    let modu = uv(m).max(1);
    let mut result: u64 = 1;
    while exp > 0 {
        if exp & 1 == 1 { result = (result * base) % modu; }
        base = (base * base) % modu;
        exp >>= 1;
    }
    jn(result.to_string())
}
pub fn math_is_prime(t: &str) -> String {
    let n = uv(t);
    if n < 2 { return j("false"); }
    let mut i = 2u64;
    while i * i <= n { if n % i == 0 { return j("false"); } i += 1; }
    j("true")
}
pub fn math_next_prime(t: &str) -> String {
    let mut n = uv(t) + 1;
    loop {
        let prime = if n < 2 { false } else {
            let mut i = 2u64; let mut p = true;
            while i * i <= n { if n % i == 0 { p = false; break; } i += 1; }
            p
        };
        if prime { return jn(n.to_string()); }
        n += 1;
    }
}
pub fn math_num_divisors(t: &str) -> String {
    let n = uv(t);
    if n == 0 { return jn("0".to_string()); }
    let mut c = 0u64;
    let mut i = 1u64;
    while i * i <= n {
        if n % i == 0 { c += 1; if i != n / i { c += 1; } }
        i += 1;
    }
    jn(c.to_string())
}
pub fn math_digital_root(t: &str) -> String {
    let mut n = uv(t);
    if n == 0 { return jn("0".to_string()); }
    jn((1 + (n - 1) % 9).to_string())
}
pub fn math_is_perfect(t: &str) -> String {
    let n = uv(t);
    if n < 2 { return j("false"); }
    let mut sum = 1u64;
    let mut i = 2u64;
    while i * i <= n {
        if n % i == 0 { sum += i; if i != n / i { sum += n / i; } }
        i += 1;
    }
    j(if sum == n { "true" } else { "false" })
}
pub fn math_nthfib(t: &str) -> String {
    let n = iv(t);
    if n <= 0 { return jn("0".to_string()); }
    let (mut a, mut b) = (0u64, 1u64);
    for _ in 1..n as u64 { let c = a + b; a = b; b = c; }
    jn(b.to_string())
}

// ===================== 日志 / 配置分析 =====================
pub fn log_level(t: &str) -> String {
    let lower = t.to_ascii_lowercase();
    let lvl = if lower.contains("fatal") || lower.contains("panic") { "FATAL" }
        else if lower.contains("error") { "ERROR" }
        else if lower.contains("warn") { "WARN" }
        else if lower.contains("debug") { "DEBUG" }
        else if lower.contains("trace") { "TRACE" }
        else if lower.contains("info") { "INFO" }
        else { "未知" };
    j(lvl)
}
pub fn log_ip_count(t: &str) -> String {
    let mut set: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for word in t.split_whitespace() {
        let w = word.trim_matches(|c| c == ':' || c == '[' || c == ']' || c == ',' || c == '(' || c == ')');
        let ok = w.split('.').count() == 4 && w.split('.').all(|p| p.parse::<u32>().map(|v| v <= 255).unwrap_or(false));
        if ok { set.insert(w); }
    }
    jn(set.len().to_string())
}
pub fn log_error_lines(t: &str) -> String {
    let mut n = 0;
    for line in t.lines() {
        let l = line.to_ascii_lowercase();
        if l.contains("error") || l.contains("exception") || l.contains(" failed") { n += 1; }
    }
    jn(n.to_string())
}
pub fn log_ts_count(t: &str) -> String {
    // 统计含 ISO 风格时间戳的行
    let mut n = 0;
    for line in t.lines() {
        let l = line.trim();
        if l.len() >= 10 && l.as_bytes()[4] == b'-' && l.as_bytes()[7] == b'-' { n += 1; }
    }
    jn(n.to_string())
}
pub fn log_stacktrace_lines(t: &str) -> String {
    let mut n = 0;
    for line in t.lines() {
        if line.trim_start().starts_with("at ") || line.trim_start().starts_with("    at ") { n += 1; }
    }
    jn(n.to_string())
}
pub fn cfg_comment_lines(t: &str) -> String {
    jn(t.lines().filter(|l| { let x = l.trim_start(); x.starts_with('#') || x.starts_with("//") || x.starts_with(';') }).count().to_string())
}
pub fn cfg_brace_balance(t: &str) -> String {
    let mut open = 0i64;
    for c in t.chars() {
        match c { '{' => open += 1, '}' => open -= 1, _ => {} }
    }
    jn(open.to_string())
}
pub fn cfg_section_count(t: &str) -> String {
    jn(t.lines().filter(|l| { let x = l.trim(); x.starts_with('[') && x.ends_with(']') }).count().to_string())
}
pub fn cfg_equals_count(t: &str) -> String {
    jn(t.lines().filter(|l| l.trim().contains('=')).count().to_string())
}
pub fn json_balanced(t: &str) -> String {
    let mut open = 0i64;
    let mut in_str = false;
    let mut esc = false;
    for c in t.chars() {
        if in_str {
            if esc { esc = false; }
            else if c == '\\' { esc = true; }
            else if c == '"' { in_str = false; }
        } else {
            match c {
                '"' => in_str = true,
                '{' | '[' => open += 1,
                '}' | ']' => open -= 1,
                _ => {}
            }
        }
        if open < 0 { return j("false"); }
    }
    j(if open == 0 && !in_str { "true" } else { "false" })
}
pub fn log_line_lengths(t: &str) -> String {
    let max = t.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let avg = if t.lines().count() == 0 { 0 } else { t.lines().map(|l| l.chars().count()).sum::<usize>() / t.lines().count() };
    format!("{{\"result\":\"max:{},avg:{}\"}}", max, avg)
}
pub fn uri_host(t: &str) -> String {
    let after = match t.find("://") { Some(i) => &t[i + 3..], None => t };
    let host = after.split(['/', ':', '?', '#']).next().unwrap_or("");
    j(host)
}
pub fn pass_common_weak(t: &str) -> String {
    let weak = ["123456", "password", "12345678", "qwerty", "abc123", "111111", "123123", "admin", "iloveyou", "letmein", "666666", "888888"];
    j(if weak.contains(&t.trim()) { "true" } else { "false" })
}
pub fn math_is_coprime(t: &str, a: &str) -> String {
    let g = { let (mut a1, mut b1) = (uv(t), uv(a)); while b1 != 0 { let m = a1 % b1; a1 = b1; b1 = m; } a1 };
    j(if g == 1 { "true" } else { "false" })
}
pub fn math_triangle_type(t: &str, a: &str, b: &str) -> String {
    let mut s = [uv(t), uv(a), uv(b)];
    s.sort();
    if s[0] + s[1] <= s[2] { return j("非三角形"); }
    let sq = |x: u64| x * x;
    if sq(s[0]) + sq(s[1]) == sq(s[2]) { j("直角") }
    else if sq(s[0]) + sq(s[1]) < sq(s[2]) { j("钝角") }
    else if s[0] == s[1] && s[1] == s[2] { j("等边") }
    else if s[0] == s[1] || s[1] == s[2] { j("等腰") }
    else { j("锐角普通") }
}
pub fn log_warn_lines(t: &str) -> String {
    jn(t.lines().filter(|l| l.to_ascii_lowercase().contains("warn")).count().to_string())
}
pub fn log_info_lines(t: &str) -> String {
    jn(t.lines().filter(|l| l.to_ascii_lowercase().contains("info")).count().to_string())
}
pub fn sec_control_chars(t: &str) -> String {
    jn(t.chars().filter(|c| c.is_control() && *c != '\n' && *c != '\t').count().to_string())
}