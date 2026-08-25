//! 内置工具 · 字符串与文本处理（`ops5`）。
//! 全部为纯函数，随调随算、无状态。每个工具有各自独立的功能，不做批量占位。
//! 命名不与 ops/ops2/ops3/ops4 重复。

use crate::json;

fn j(s: &str) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(s))
}
fn jn(n: String) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(&n))
}
fn nv(t: &str) -> f64 {
    t.trim().parse::<f64>().unwrap_or_default()
}
fn iv(t: &str) -> i64 {
    nv(t) as i64
}

// ---- 长度 / 截取 / 访问 ----
pub fn str_len(t: &str) -> String { jn(t.chars().count().to_string()) }
pub fn str_char_at(t: &str, i: &str) -> String {
    let idx = iv(i).max(0) as usize;
    match t.chars().nth(idx) { Some(c) => j(&c.to_string()), None => j("(越界)") }
}
pub fn str_slice(t: &str, s: &str, e: &str) -> String {
    let chars: Vec<char> = t.chars().collect();
    let i = iv(s).max(0) as usize;
    let mut en = if e.is_empty() { chars.len() } else { iv(e) as usize };
    en = en.min(chars.len());
    if i >= en { return j(""); }
    j(&chars[i..en].iter().collect::<String>())
}
pub fn str_first(t: &str) -> String { j(&t.chars().next().map(|c| c.to_string()).unwrap_or_default()) }
pub fn str_last(t: &str) -> String { j(&t.chars().next_back().map(|c| c.to_string()).unwrap_or_default()) }
pub fn str_first_n(t: &str, n: &str) -> String {
    let k = iv(n).max(0) as usize;
    j(&t.chars().take(k).collect::<String>())
}
pub fn str_last_n(t: &str, n: &str) -> String {
    let k = iv(n).max(0) as usize;
    let chars: Vec<char> = t.chars().collect();
    if k >= chars.len() { return j(t); }
    j(&chars[chars.len() - k..].iter().collect::<String>())
}

// ---- 大小写转换 ----
pub fn str_upper(t: &str) -> String { j(&t.to_uppercase()) }
pub fn str_lower(t: &str) -> String { j(&t.to_lowercase()) }
pub fn str_title(t: &str) -> String {
    let out = t.split_whitespace()
        .map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    j(&out)
}
pub fn str_swap_case(t: &str) -> String {
    let out: String = t.chars().map(|c| {
        if c.is_uppercase() { c.to_lowercase().to_string() }
        else if c.is_lowercase() { c.to_uppercase().to_string() }
        else { c.to_string() }
    }).collect();
    j(&out)
}
pub fn str_capitalize(t: &str) -> String {
    let mut cs = t.chars();
    match cs.next() {
        Some(f) => j(&(f.to_uppercase().collect::<String>() + cs.as_str())),
        None => j(""),
    }
}
pub fn str_sentence(t: &str) -> String {
    let out = t.split('.').map(|s| { let s = s.trim(); if s.is_empty() { String::new() } else { let mut c = s.chars(); let f = c.next().unwrap(); f.to_uppercase().collect::<String>() + c.as_str().to_lowercase().as_str() } }).collect::<Vec<_>>().join(". ");
    j(&out.trim())
}

// ---- 修剪 / 填充 / 对齐 ----
pub fn str_trim(t: &str) -> String { j(t.trim()) }
pub fn str_trim_start(t: &str) -> String { j(t.trim_start()) }
pub fn str_trim_end(t: &str) -> String { j(t.trim_end()) }
pub fn str_ltrim_char(t: &str, ch: &str) -> String {
    let c = ch.chars().next().unwrap_or(' ');
    j(t.trim_start_matches(c))
}
pub fn str_rtrim_char(t: &str, ch: &str) -> String {
    let c = ch.chars().next().unwrap_or(' ');
    j(t.trim_end_matches(c))
}
pub fn str_trim_char(t: &str, ch: &str) -> String {
    let c = ch.chars().next().unwrap_or(' ');
    j(t.trim_matches(c))
}
pub fn str_pad_left(t: &str, n: &str, ch: &str) -> String {
    let k = iv(n).max(0) as usize;
    let len = t.chars().count();
    if k <= len { return j(t); }
    let pad = ch.chars().next().unwrap_or(' ');
    let mut out = String::with_capacity(k);
    for _ in 0..(k - len) { out.push(pad); }
    out.push_str(t);
    j(&out)
}
pub fn str_pad_right(t: &str, n: &str, ch: &str) -> String {
    let k = iv(n).max(0) as usize;
    let len = t.chars().count();
    if k <= len { return j(t); }
    let pad = ch.chars().next().unwrap_or(' ');
    let mut out = String::from(t);
    for _ in 0..(k - len) { out.push(pad); }
    j(&out)
}
pub fn str_zfill(t: &str, n: &str) -> String {
    let k = iv(n).max(0) as usize;
    let len = t.chars().count();
    if k <= len { return j(t); }
    let sign: String = if let Some(st) = t.chars().next() { if st == '-' || st == '+' { st.to_string() } else { String::new() } } else { String::new() };
    let sign_len = sign.chars().count();
    let digits = t.trim_start_matches(['-', '+']);
    let mut out = sign;
    for _ in 0..(k - len.max(sign_len)) { out.push('0'); }
    out.push_str(digits);
    j(&out)
}
pub fn str_center(t: &str, n: &str, ch: &str) -> String {
    let k = iv(n).max(0) as usize;
    let len = t.chars().count();
    if k <= len { return j(t); }
    let pad = ch.chars().next().unwrap_or(' ');
    let total = k - len;
    let left = total / 2;
    let right = total - left;
    let mut out = String::new();
    for _ in 0..left { out.push(pad); }
    out.push_str(t);
    for _ in 0..right { out.push(pad); }
    j(&out)
}
pub fn str_truncate(t: &str, n: &str) -> String {
    let k = iv(n).max(0) as usize;
    if t.chars().count() <= k { return j(t); }
    let head: String = t.chars().take(k.max(1) - 1).collect();
    j(&format!("{}…", head))
}

// ---- 查找 / 包含 / 计数 ----
pub fn str_contains(t: &str, needle: &str) -> String {
    j(if needle.is_empty() { "true" } else if t.contains(needle) { "true" } else { "false" })
}
pub fn str_starts_with(t: &str, pre: &str) -> String {
    j(if pre.is_empty() { "true" } else if t.starts_with(pre) { "true" } else { "false" })
}
pub fn str_ends_with(t: &str, suf: &str) -> String {
    j(if suf.is_empty() { "true" } else if t.ends_with(suf) { "true" } else { "false" })
}
pub fn str_index_of(t: &str, needle: &str) -> String {
    match t.find(needle) { Some(i) => jn(i.to_string()), None => j("-1") }
}
pub fn str_last_index(t: &str, needle: &str) -> String {
    match t.rfind(needle) { Some(i) => jn(i.to_string()), None => j("-1") }
}
pub fn str_count(t: &str, needle: &str) -> String {
    if needle.is_empty() { return j("0"); }
    let mut count = 0usize;
    let mut rest = t;
    while let Some(i) = rest.find(needle) { count += 1; rest = &rest[i + needle.len()..]; }
    jn(count.to_string())
}

// ---- 替换 / 删除 / 拆分 / 重组 ----
pub fn str_replace(t: &str, from: &str, to_rep: &str) -> String {
    if from.is_empty() { return j(t); }
    j(&t.replace(from, to_rep))
}
pub fn str_remove(t: &str, sub: &str) -> String {
    j(&t.to_string().replacen(sub, "", 1))
}
pub fn str_remove_all(t: &str, sub: &str) -> String {
    j(&t.to_string().replace(sub, ""))
}
pub fn str_delete(t: &str, s: &str, e: &str) -> String {
    let chars: Vec<char> = t.chars().collect();
    let i = iv(s).max(0) as usize;
    let mut en = if e.is_empty() { i } else { iv(e) as usize };
    en = en.min(chars.len());
    if i >= en { return j(t); }
    let mut out: String = chars[..i].iter().collect();
    out.push_str(&chars[en..].iter().collect::<String>());
    j(&out)
}
pub fn str_insert(t: &str, at: &str, ins: &str) -> String {
    let chars: Vec<char> = t.chars().collect();
    let i = iv(at).max(0).min(chars.len() as i64) as usize;
    let mut out: String = chars[..i].iter().collect();
    out.push_str(ins);
    out.push_str(&chars[i..].iter().collect::<String>());
    j(&out)
}
pub fn str_repeat(t: &str, n: &str) -> String {
    let k = iv(n).max(0).min(10000) as usize;
    j(&t.repeat(k))
}
pub fn str_rev(t: &str) -> String {
    j(&t.chars().rev().collect::<String>())
}
pub fn str_split(t: &str, sep: &str) -> String {
    let parts: Vec<&str> = if sep.is_empty() { t.split(|_: char| false).collect() } else { t.split(sep).collect() };
    format!("{{\"result\":[{}]}}", parts.iter().map(|p| format!("\"{}\"", json::jesc(p))).collect::<Vec<_>>().join(","))
}
pub fn str_join(a: &str, sep: &str, b: &str) -> String {
    j(&format!("{}{}{}", a, sep, b))
}

// ---- 单词 / 统计 ----
pub fn str_word_count(t: &str) -> String { jn(t.split_whitespace().count().to_string()) }
pub fn str_line_count(t: &str) -> String {
    let n = if t.is_empty() { 0 } else { t.lines().count() };
    jn(n.to_string())
}
pub fn str_number_lines(t: &str) -> String {
    let out = t.lines().enumerate().map(|(i, l)| format!("{:>4}  {}", i + 1, l)).collect::<Vec<_>>().join("\n");
    j(&out)
}
pub fn str_first_word(t: &str) -> String {
    j(&t.split_whitespace().next().unwrap_or_default())
}
pub fn str_last_word(t: &str) -> String {
    j(&t.split_whitespace().next_back().unwrap_or_default())
}

// ---- 字符构成分析 ----
pub fn str_alpha_count(t: &str) -> String {
    jn(t.chars().filter(|c| c.is_alphabetic()).count().to_string())
}
pub fn str_digit_count(t: &str) -> String {
    jn(t.chars().filter(|c| c.is_ascii_digit()).count().to_string())
}
pub fn str_space_count(t: &str) -> String {
    jn(t.chars().filter(|c| c.is_whitespace()).count().to_string())
}
pub fn str_punct_count(t: &str) -> String {
    jn(t.chars().filter(|c| c.is_ascii_punctuation()).count().to_string())
}
pub fn str_vowel_count(t: &str) -> String {
    jn(t.to_lowercase().chars().filter(|c| matches!(c, 'a'|'e'|'i'|'o'|'u')).count().to_string())
}
pub fn str_unique_chars(t: &str) -> String {
    let mut set: Vec<char> = Vec::new();
    for c in t.chars() { if !set.contains(&c) { set.push(c); } }
    j(&set.iter().collect::<String>())
}

// ---- 判定 ----
pub fn str_is_empty(t: &str) -> String { j(if t.is_empty() { "true" } else { "false" }) }
pub fn str_is_digits(t: &str) -> String {
    j(if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) { "true" } else { "false" })
}
pub fn str_is_letters(t: &str) -> String {
    j(if !t.is_empty() && t.chars().all(|c| c.is_alphabetic()) { "true" } else { "false" })
}
pub fn str_is_alnum(t: &str) -> String {
    j(if !t.is_empty() && t.chars().all(|c| c.is_alphanumeric()) { "true" } else { "false" })
}
pub fn str_is_upper(t: &str) -> String {
    j(if t.chars().any(|c| c.is_lowercase()) { "false" } else { "true" })
}
pub fn str_is_lower(t: &str) -> String {
    j(if t.chars().any(|c| c.is_uppercase()) { "false" } else { "true" })
}
pub fn str_is_space(t: &str) -> String {
    j(if !t.is_empty() && t.chars().all(|c| c.is_whitespace()) { "true" } else { "false" })
}
pub fn str_is_palindrome(t: &str) -> String {
    let s: String = t.chars().filter(|c| !c.is_whitespace()).map(|c| c.to_lowercase().next().unwrap()).collect();
    let r: String = s.chars().rev().collect();
    j(if s == r { "true" } else { "false" })
}

// ---- 命名风格转换 ----
pub fn str_to_snake(t: &str) -> String {
    let mut out = String::new();
    for (i, c) in t.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 { out.push('_'); }
            out.push(c.to_lowercase().next().unwrap());
        } else if c == '-' || c == ' ' { out.push('_'); }
        else { out.push(c); }
    }
    j(&out)
}
pub fn str_to_kebab(t: &str) -> String {
    str_to_snake(t).replace('_', "-").to_string()
}
pub fn str_to_camel(t: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in t.chars() {
        if c == '_' || c == '-' || c == ' ' { up = true; }
        else if up { out.push_str(&c.to_uppercase().to_string()); up = false; }
        else { out.push(c); }
    }
    j(&out)
}
pub fn str_to_pascal(t: &str) -> String {
    let camel = str_to_camel(t).trim_start_matches('"').trim_end_matches('"').to_string();
    let mut cs = camel.chars();
    let p = match cs.next() {
        Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
        None => String::new(),
    };
    j(&p)
}

// ---- 距离 / 相似度 ----
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    if n == 0 { return m; }
    if m == 0 { return n; }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i-1] == b[j-1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j-1] + 1).min(prev[j-1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}
pub fn str_edit_distance(t: &str, o: &str) -> String {
    jn(levenshtein(t, o).to_string())
}
pub fn str_similarity(t: &str, o: &str) -> String {
    let d = levenshtein(t, o);
    let m = t.chars().count().max(o.chars().count());
    let sim = if m == 0 { 1.0 } else { 1.0 - (d as f64) / (m as f64) };
    jn(format!("{:.4}", sim))
}

// ---- 引用 / 转义 ----
pub fn str_quote(t: &str) -> String { j(&format!("\"{}\"", t)) }
pub fn str_unquote(t: &str) -> String {
    let s = t.trim();
    if s.len() >= 2 && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))) {
        j(&s[1..s.len()-1])
    } else { j(t) }
}
pub fn str_escape(t: &str) -> String {
    j(&t.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t").replace('\r', "\\r"))
}
pub fn str_unescape(t: &str) -> String {
    j(&t.replace("\\n", "\n").replace("\\t", "\t").replace("\\r", "\r").replace("\\\\", "\\"))
}
pub fn str_indent(t: &str, n: &str) -> String {
    let k = iv(n).max(0).min(64) as usize;
    let pad = " ".repeat(k);
    let out = t.lines().map(|l| format!("{}{}", pad, l)).collect::<Vec<_>>().join("\n");
    j(&out)
}