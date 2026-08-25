//! 内置工具 · 文件 / 路径 / 权限 / 磁盘 / 进程（`ops9`）。
//! 全部为纯字符串/数值计算，无状态、不落盘，命名不与既有模块冲突。

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

// ===================== 路径计算 =====================
pub fn path_basename(t: &str) -> String {
    let p = t.trim_end_matches('/');
    match p.rsplit('/').next() {
        Some(x) if !x.is_empty() => j(x),
        _ => j("/"),
    }
}
pub fn path_dirname(t: &str) -> String {
    let p = t.trim_end_matches('/');
    match p.rfind('/') {
        Some(i) if i > 0 => j(&p[..i]),
        Some(_) => j("/"),
        None => j("."),
    }
}
pub fn path_ext(t: &str) -> String {
    let b = path_basename(t);
    let b = b.trim().trim_matches('"');
    match b.rfind('.') {
        Some(i) if i > 0 => j(&b[i + 1..]),
        _ => j(""),
    }
}
pub fn path_ext_set(t: &str, e: &str) -> String {
    let e = e.trim();
    match t.rfind('.') {
        Some(i) if i > 0 => j(&format!("{}.{}", &t[..i], e)),
        _ => j(&format!("{}.{}", t, e)),
    }
}
pub fn path_stem(t: &str) -> String {
    let b = path_basename(t);
    let b = b.trim().trim_matches('"');
    match b.rfind('.') {
        Some(i) if i > 0 => j(&b[..i]),
        _ => j(b),
    }
}
pub fn path_join(t: &str, a: &str) -> String {
    let t = t.trim_end_matches('/');
    let a = a.trim_start_matches('/');
    j(&format!("{}/{}", t, a))
}
pub fn path_normalize(t: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in t.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if out.last().map_or(true, |&s| s == "..") { out.push(".."); } else { out.pop(); }
            }
            s => out.push(s),
        }
    }
    let body = out.join("/");
    if t.starts_with('/') { j(&format!("/{}", body)) } else if body.is_empty() { j(".") } else { j(&body) }
}
pub fn path_clean(t: &str) -> String {
    let mut segs: Vec<&str> = t.split('/').filter(|s| !s.is_empty()).collect();
    if segs.is_empty() { return j("/"); }
    if segs.last() == Some(&"") { segs.pop(); }
    // 去重连续斜杠（filter 已处理空段）
    if t.starts_with('/') { j(&format!("/{}", segs.join("/"))) } else { j(&segs.join("/")) }
}
pub fn path_is_abs(t: &str) -> String {
    j(if t.starts_with('/') { "true" } else { "false" })
}
pub fn path_is_rel(t: &str) -> String {
    let s = t.trim();
    j(if s.starts_with('/') || s.starts_with("\\") || s.contains("://") { "false" } else { "true" })
}
pub fn path_depth(t: &str) -> String {
    let n = t.split('/').filter(|s| !s.is_empty()).count();
    jn(n.to_string())
}
pub fn path_split(t: &str) -> String {
    let segs: Vec<&str> = t.split('/').filter(|s| !s.is_empty()).collect();
    let arr: Vec<String> = segs.iter().map(|&s| format!("\"{}\"", json::jesc(s))).collect();
    format!("{{\"result\":[{}]}}", arr.join(","))
}
pub fn path_parent_all(t: &str) -> String {
    let mut segs: Vec<&str> = t.split('/').filter(|s| !s.is_empty()).collect();
    let mut out: Vec<String> = Vec::new();
    while segs.len() > 1 {
        segs.pop();
        let p = if t.starts_with('/') { format!("/{}", segs.join("/")) } else { segs.join("/") };
        out.push(format!("\"{}\"", json::jesc(&p)));
    }
    format!("{{\"result\":[{}]}}", out.join(","))
}
pub fn path_common_prefix(t: &str, a: &str) -> String {
    let s1: Vec<&str> = t.trim_start_matches('/').split('/').collect();
    let s2: Vec<&str> = a.trim_start_matches('/').split('/').collect();
    let mut common: Vec<&str> = Vec::new();
    for (x, y) in s1.iter().zip(s2.iter()) {
        if x == y { common.push(x); } else { break; }
    }
    let body = common.join("/");
    if body.is_empty() { j("") } else if t.starts_with('/') { j(&format!("/{}", body)) } else { j(&body) }
}
pub fn path_is_within(t: &str, a: &str) -> String {
    // t 是否位于目录 a 之下
    let base = path_normalize(a);
    let full = path_normalize(t);
    let bs = base.trim().trim_matches('"');
    let fs = full.trim().trim_matches('"');
    j(if fs.starts_with(&format!("{}/", bs)) { "true" } else { "false" })
}
pub fn path_relativize(t: &str, a: &str) -> String {
    // 给出 t——目录, a——目标；返回 a 相对 t 的路径。若不在包含关系内，尽力计算公共前缀差值。
    let base: Vec<&str> = t.split('/').filter(|s| !s.is_empty()).collect();
    let target: Vec<&str> = a.split('/').filter(|s| !s.is_empty()).collect();
    let mut i = 0;
    while i < base.len() && i < target.len() && base[i] == target[i] { i += 1; }
    let mut ups = vec![".."; base.len() - i];
    ups.extend_from_slice(&target[i..]);
    let r = ups.join("/");
    j(if r.is_empty() { "." } else { &r })
}
pub fn path_ensure_slash(t: &str) -> String {
    let s = t.trim();
    if s.is_empty() { return j("/"); }
    if s.ends_with('/') { return j(s); }
    j(&format!("{}/", s))
}
pub fn path_trim_slash(t: &str) -> String {
    let s = t.trim_matches('/');
    j(if s.is_empty() { "/" } else { s })
}
pub fn path_sep_count(t: &str) -> String {
    jn(t.matches('/').count().to_string())
}
pub fn path_home_expand(t: &str) -> String {
    // 将 ~ 前缀规范为 Linux 风格绝对路径（纯文本约定 /root/）
    let s = t.trim_start();
    if let Some(rest) = s.strip_prefix('~') {
        j(&format!("/root{}", rest))
    } else {
        j(s)
    }
}

// ===================== 权限计算 =====================
fn perm_val(c: char) -> u64 {
    match c {
        'r' | 'R' => 4,
        'w' | 'W' => 2,
        'x' | 'X' => 1,
        's' | 'S' | 't' | 'T' => 0, // 特殊位单独处理
        _ => 0,
    }
}
pub fn perm_rwx(t: &str) -> String {
    // 输入 3 位八进制（或其十进制等价），输出 rwx 表示
    let n = iv(t);
    if n < 0 || n > 777 { return j("无效权限"); }
    let (a, b, c) = ((n / 100) % 10, (n / 10) % 10, n % 10);
    let sym = |x: i64| -> String {
        let mut s = String::new();
        s.push(if x & 4 != 0 { 'r' } else { '-' });
        s.push(if x & 2 != 0 { 'w' } else { '-' });
        s.push(if x & 1 != 0 { 'x' } else { '-' });
        s
    };
    j(&format!("{}{}{}", sym(a), sym(b), sym(c)))
}
pub fn perm_octal(t: &str) -> String {
    // 输入 rwx 串（9 符号），输出 3 位八进制
    let s: Vec<char> = t.trim().chars().collect();
    if s.len() != 9 { return j("需要 9 位 rwx 串"); }
    let grp = |g: &[char]| -> i64 {
        let mut v = 0i64;
        if g[0] == 'r' { v |= 4; }
        if g[1] == 'w' { v |= 2; }
        if g[2] == 'x' { v |= 1; }
        v
    };
    let a = grp(&s[0..3]);
    let b = grp(&s[3..6]);
    let c = grp(&s[6..9]);
    jn(format!("{}{}{}", a, b, c))
}
pub fn perm_symbol_sum(t: &str) -> String {
    // 解析单个 rwx 三位，返回其八进制权值和
    let mut v = 0i64;
    for ch in t.trim().chars() {
        if ch == 'r' { v += 4; }
        if ch == 'w' { v += 2; }
        if ch == 'x' { v += 1; }
    }
    jn(v.to_string())
}
pub fn perm_sticky(t: &str) -> String {
    // 输入权限数字（含4位可选前缀），是否有 setuid/setgid/sticky
    let n = iv(t).min(7777);
    let special = n / 1000;
    let mut flags = String::new();
    if special & 4 != 0 { flags.push_str("setuid "); }
    if special & 2 != 0 { flags.push_str("setgid "); }
    if special & 1 != 0 { flags.push_str("sticky "); }
    if flags.is_empty() { j("无") } else { j(flags.trim()) }
}
pub fn perm_like(t: &str, a: &str) -> String {
    // t: 用户掩码, a: 期望掩码, 比较是否 t ⊆ a
    let mask = |s: &str| -> u64 { s.chars().filter(|&c| "rwx".contains(c)).fold(0u64, |acc, c| acc + perm_val(c)) };
    let t = mask(t.trim());
    let a = mask(a.trim());
    j(if a & t == t { "true" } else { "false" })
}

// ===================== 大小 / 磁盘换算 =====================
pub fn size_in_bytes(t: &str) -> String {
    // 人类可读 → 字节，如 "1.5K" "2M" "3G"
    let s = t.trim();
    let (num, mult) = if let Some(v) = s.strip_suffix("K").or_else(|| s.strip_suffix("k")) { (v, 1024u64) }
        else if let Some(v) = s.strip_suffix("M").or_else(|| s.strip_suffix("m")) { (v, 1048576u64) }
        else if let Some(v) = s.strip_suffix("G").or_else(|| s.strip_suffix("g")) { (v, 1073741824u64) }
        else if let Some(v) = s.strip_suffix("T").or_else(|| s.strip_suffix("t")) { (v, 1099511627776u64) }
        else { (s, 1u64) };
    let n: f64 = num.trim().parse().unwrap_or(0.0);
    jn((n as u64 * mult).to_string())
}
pub fn size_auto(t: &str) -> String {
    // 字节 → 自动单位（B/K/M/G/T）
    let b = uv(t);
    let units = ["B", "K", "M", "G", "T"];
    let mut val = b as f64;
    let mut u = 0usize;
    while val >= 1024.0 && u < units.len() - 1 { val /= 1024.0; u += 1; }
    if u == 0 { j(&format!("{}{}", b, units[0])) }
    else { j(&format!("{:.2}{}", val, units[u])) }
}
pub fn size_compare(t: &str, a: &str) -> String {
    // t 与 a 均为可读大小，比较大小返回 -1/0/1
    let x = sz_num(t);
    let y = sz_num(a);
    jn(match x.partial_cmp(&y) {
        Some(std::cmp::Ordering::Less) => "-1".to_string(),
        Some(std::cmp::Ordering::Greater) => "1".to_string(),
        _ => "0".to_string(),
    })
}
fn sz_num(s: &str) -> f64 {
    let s = s.trim();
    let (num, mult) = if let Some(v) = s.strip_suffix('K').or_else(|| s.strip_suffix('k')) { (v, 1024.0) }
        else if let Some(v) = s.strip_suffix('M').or_else(|| s.strip_suffix('m')) { (v, 1048576.0) }
        else if let Some(v) = s.strip_suffix('G').or_else(|| s.strip_suffix('g')) { (v, 1073741824.0) }
        else if let Some(v) = s.strip_suffix('T').or_else(|| s.strip_suffix('t')) { (v, 1099511627776.0) }
        else { (s, 1.0) };
    num.trim().parse::<f64>().unwrap_or(0.0) * mult
}
pub fn disk_fill_percent(t: &str, a: &str) -> String {
    // t=用量, a=总量 → 百分比
    let used = uv(t);
    let total = uv(a);
    if total == 0 { return jn("0".to_string()); }
    jn(format!("{:.1}", used as f64 / total as f64 * 100.0))
}
pub fn disk_free_est(t: &str, a: &str) -> String {
    // t=总量, a=用量 → 剩余
    let total = uv(t);
    let used = uv(a);
    jn(total.checked_sub(used).unwrap_or(0).to_string())
}
pub fn block_count(t: &str, block: &str) -> String {
    // 文件总字节 / 块大小 → 块数（向上取整）
    let bytes = uv(t);
    let bs = uv(block).max(1);
    jn(((bytes + bs - 1) / bs).to_string())
}

// ===================== 文本 / 文件内容统计 =====================
pub fn text_line_count(t: &str) -> String {
    if t.is_empty() { return jn("0".to_string()); }
    jn(t.lines().count().to_string())
}
pub fn text_bytes(t: &str) -> String { jn(t.len().to_string()) }
pub fn text_words(t: &str) -> String {
    let n = t.split_whitespace().count();
    jn(n.to_string())
}
pub fn text_nonws(t: &str) -> String {
    let n = t.chars().filter(|c| !c.is_whitespace()).count();
    jn(n.to_string())
}
pub fn text_ntabs(t: &str) -> String { jn(t.matches('\t').count().to_string()) }
pub fn text_nlines_nl(t: &str) -> String { jn(t.matches('\n').count().to_string()) }
pub fn text_line_len(t: &str) -> String {
    // 返回包含换行文本中的最长行字符数
    jn(t.lines().map(|l| l.chars().count()).max().unwrap_or(0).to_string())
}
pub fn text_contains_cn(t: &str) -> String {
    j(if t.chars().any(|c| (c as u32) >= 0x4E00 && (c as u32) <= 0x9FFF) { "true" } else { "false" })
}
pub fn text_ascii_percent(t: &str) -> String {
    let total = t.chars().count();
    if total == 0 { return jn("0".to_string()); }
    let ascii = t.chars().filter(|c| c.is_ascii()).count();
    jn(format!("{:.1}", ascii as f64 / total as f64 * 100.0))
}
pub fn text_ident_lines(t: &str) -> String {
    jn(t.lines().filter(|l| l.chars().next().map_or(false, |c| c.is_whitespace())).count().to_string())
}
pub fn text_blank_lines(t: &str) -> String {
    jn(t.lines().filter(|l| l.trim().is_empty()).count().to_string())
}
pub fn text_average_wlen(t: &str) -> String {
    let words: Vec<&str> = t.split_whitespace().collect();
    if words.is_empty() { return jn("0".to_string()); }
    let sum: usize = words.iter().map(|w| w.chars().count()).sum();
    jn(format!("{:.1}", sum as f64 / words.len() as f64))
}

// ===================== 进程 / 负载字符串分析 =====================
pub fn proc_cpu_percent(t: &str, a: &str) -> String {
    // t=进程cpu时间(秒，可含小数), a=墙钟(秒) → 占用%
    let tc: f64 = t.trim().parse().unwrap_or(0.0);
    let wall: f64 = a.trim().parse::<f64>().unwrap_or(0.0).max(0.0001);
    jn(format!("{:.2}", tc * 100.0 / wall))
}
pub fn proc_vm_human(t: &str) -> String {
    size_auto(t) // 复用：KB → 人类可读
}
pub fn load_interpret(t: &str) -> String {
    // 负载值 → 中文描述
    let v = t.trim().parse::<f64>().unwrap_or(0.0);
    let d = if v < 1.0 { "空闲" } else if v < 2.0 { "轻度" } else if v < 4.0 { "忙碌" } else if v < 8.0 { "高" } else { "过高，需关注" };
    j(d)
}
pub fn uptime_days(t: &str) -> String {
    let s = uv(t);
    jn(format!("{:.1}", s as f64 / 86400.0))
}
pub fn path_is_root(t: &str) -> String {
    j(if t.trim() == "/" { "true" } else { "false" })
}
pub fn path_regex_escape(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        if r".+*?^$()[]{}|\".contains(c) { out.push('\\'); }
        out.push(c);
    }
    j(&out)
}
pub fn path_indexed(t: &str, n: &str) -> String {
    // 取路径第 n 层（从0起）
    let idx = iv(n).max(0) as usize;
    let segs: Vec<&str> = t.split('/').filter(|s| !s.is_empty()).collect();
    match segs.get(idx) { Some(&s) => j(s), None => j("") }
}
pub fn path_tail_n(t: &str, n: &str) -> String {
    // 取路径末尾 n 个段
    let cnt = iv(n).max(0) as usize;
    let segs: Vec<&str> = t.split('/').filter(|s| !s.is_empty()).collect();
    let start = segs.len().saturating_sub(cnt);
    let tail: Vec<String> = segs[start..].iter().map(|&s| json::jesc(s)).collect();
    format!("{{\"result\":[{}]}}", tail.join(","))
}
pub fn path_has_hidden(t: &str) -> String {
    j(if t.split('/').any(|s| s.starts_with('.') && s.len() > 1) { "true" } else { "false" })
}
pub fn path_double_dots(t: &str) -> String {
    jn(t.split('/').filter(|&s| s == "..").count().to_string())
}
pub fn perm_extended(t: &str) -> String {
    // 输入 3~4 位八进制（4位时首位为特殊位），输出完整 rwx+特殊标记
    let n = iv(t);
    if n < 0 { return j("无效"); }
    let special = if n >= 1000 { n / 1000 } else { 0 };
    let base = n % 1000;
    let sym = |x: i64| -> String {
        let mut s = String::new();
        s.push(if x & 4 != 0 { 'r' } else { '-' });
        s.push(if x & 2 != 0 { 'w' } else { '-' });
        s.push(if x & 1 != 0 { 'x' } else { '-' });
        s
    };
    let (a, b, c) = ((base / 100) % 10, (base / 10) % 10, base % 10);
    let mut r = format!("{}{}{}", sym(a), sym(b), sym(c));
    if special & 4 != 0 { r.replace_range(2..3, if a & 1 != 0 { "s" } else { "S" }); }
    if special & 2 != 0 { r.replace_range(5..6, if b & 1 != 0 { "s" } else { "S" }); }
    if special & 1 != 0 { r.replace_range(8..9, if c & 1 != 0 { "t" } else { "T" }); }
    j(&r)
}
pub fn perm_world(t: &str) -> String {
    // 输入 3 位八进制，返回“其他人/其他组/所有人”综合判定
    let n = iv(t);
    let last = n % 10;
    let any = |x: i64| x != 0;
    if any(last) { j("可读可写或可执行") } else { j("无权限") }
}
pub fn size_bits_units(t: &str) -> String {
    let b = uv(t);
    j(&format!("{}B = {:.2}KB = {:.2}MB = {:.2}GB",
        b, b as f64 / 1024.0, b as f64 / 1048576.0, b as f64 / 1073741824.0))
}
pub fn size_ratio(t: &str, a: &str) -> String {
    let x = sz_num(t);
    let y = sz_num(a);
    if y == 0.0 { return jn("0".to_string()); }
    jn(format!("{:.3}", x / y))
}
pub fn text_synopsis(t: &str) -> String {
    // 取文本开头若干字符作摘要
    let chars: Vec<char> = t.chars().collect();
    let keep = chars.iter().take(40).collect::<String>();
    let mut s = keep;
    if chars.len() > 40 { s.push_str("…"); }
    j(&s)
}
pub fn text_longest_line(t: &str) -> String {
    let m = t.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    jn(m.to_string())
}
pub fn text_tabs_to_spaces(t: &str) -> String {
    j(&t.replace('\t', "    "))
}
pub fn proc_state_desc(t: &str) -> String {
    // 进程状态字母 → 中文
    let d = match t.trim().chars().next() {
        Some('R') => "运行",
        Some('S') => "可中断休眠",
        Some('D') => "不可中断IO",
        Some('Z') => "僵尸",
        Some('T') | Some('X') => "停止/追踪",
        Some('I') => "空闲",
        _ => "未知",
    };
    j(d)
}
pub fn proc_user_ratio(t: &str, a: &str) -> String {
    // t=用户态cpu, a=内核态cpu, 返回用户态占比%
    let usr: f64 = t.trim().parse().unwrap_or(0.0);
    let ker: f64 = a.trim().parse().unwrap_or(0.0);
    jn(format!("{:.2}", usr * 100.0 / (usr + ker).max(0.0001)))
}
pub fn mem_percent_free(t: &str) -> String {
    // t 为当前可用内存MB（占总量未知），仅作演示：返回数值的10倍作为百分比占位——改用可读描述
    let mb: f64 = t.trim().parse().unwrap_or(0.0);
    if mb >= 2048.0 { j("充足") } else if mb >= 512.0 { j("正常") } else { j("偏紧") }
}