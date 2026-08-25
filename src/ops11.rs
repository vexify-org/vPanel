//! 内置工具 · 统计 / 哈希指纹 / 频次 / 文本距离 / 业务 / 单位（`ops11`）。
//! 全部为纯函数，无状态、不依赖外部 crate。

use crate::json;
use std::collections::HashMap;

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
fn nums(t: &str) -> Vec<f64> {
    t.split([',', ';', ' ', '\n', '\t']).filter_map(|s| s.trim().parse::<f64>().ok()).collect()
}

// ===================== 统计 =====================
pub fn stat_sum(t: &str) -> String {
    jn(nums(t).iter().sum::<f64>().to_string())
}
pub fn stat_min(t: &str) -> String {
    match nums(t).into_iter().reduce(f64::min) { Some(v) => jn(v.to_string()), None => jn("0".to_string()) }
}
pub fn stat_max(t: &str) -> String {
    match nums(t).into_iter().reduce(f64::max) { Some(v) => jn(v.to_string()), None => jn("0".to_string()) }
}
pub fn stat_mean(t: &str) -> String {
    let v = nums(t);
    if v.is_empty() { return jn("0".to_string()); }
    jn(format!("{:.4}", v.iter().sum::<f64>() / v.len() as f64))
}
pub fn stat_median(t: &str) -> String {
    let mut v = nums(t);
    if v.is_empty() { return jn("0".to_string()); }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    let m = if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 };
    jn(format!("{:.4}", m))
}
pub fn stat_mode(t: &str) -> String {
    let mut fq: HashMap<i64, usize> = HashMap::new();
    for &x in &nums(t) { *fq.entry(x.round() as i64).or_insert(0) += 1; }
    let mut best: Option<(i64, usize)> = None;
    for (k, c) in fq {
        if best.map_or(false, |(_, bc)| c > bc) || best.is_none() { best = Some((k, c)); }
    }
    match best { Some((k, _)) => jn(k.to_string()), None => j("无数据") }
}
pub fn stat_range(t: &str) -> String {
    let v = nums(t);
    if v.is_empty() { return jn("0".to_string()); }
    let min = v.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    jn((max - min).to_string())
}
pub fn stat_variance(t: &str) -> String {
    let v = nums(t);
    if v.len() < 2 { return jn("0".to_string()); }
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64;
    jn(format!("{:.4}", var))
}
pub fn stat_stdev(t: &str) -> String {
    let v = nums(t);
    if v.len() < 2 { return jn("0".to_string()); }
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64;
    jn(format!("{:.4}", var.sqrt()))
}

// ===================== 哈希指纹 =====================
pub fn hash_fnv1a(t: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in t.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
    jn(format!("{:016x}", h))
}
pub fn hash_djb2(t: &str) -> String {
    let mut h: u64 = 5381;
    for b in t.as_bytes() { h = h.wrapping_mul(33).wrapping_add(*b as u64); }
    jn(format!("{:x}", h))
}
pub fn hash_elf(t: &str) -> String {
    let mut h: u32 = 0;
    for b in t.as_bytes() {
        h = (h << 4).wrapping_add(*b as u32);
        let g = h & 0xf0000000;
        if g != 0 { h ^= g >> 24; }
        h &= !g;
    }
    jn(h.to_string())
}
pub fn hash_adler(t: &str) -> String {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    const MOD: u32 = 65521;
    for byte in t.as_bytes() {
        a = (a + *byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    jn(((b << 16) | a).to_string())
}

// ===================== 频次分析 =====================
pub fn freq_char_count(t: &str, ch: &str) -> String {
    let ch = ch.chars().next().unwrap_or(' ');
    jn(t.chars().filter(|&c| c == ch).count().to_string())
}
pub fn freq_top_chars(t: &str) -> String {
    let mut fq: HashMap<char, usize> = HashMap::new();
    for c in t.chars() { if !c.is_whitespace() { *fq.entry(c).or_insert(0) += 1; } }
    let mut v: Vec<(char, usize)> = fq.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    let top: Vec<String> = v.iter().take(5).map(|(c, n)| format!("\"{}\":{}", json::jesc(&c.to_string()), n)).collect();
    format!("{{\"result\":{{{}}}}}", top.join(","))
}
pub fn letter_freq(t: &str) -> String {
    let mut fq: HashMap<char, usize> = HashMap::new();
    for c in t.chars() {
        if c.is_ascii_alphabetic() {
            let lc = c.to_ascii_lowercase();
            *fq.entry(lc).or_insert(0) += 1;
        }
    }
    let pairs: Vec<String> = (b'a'..=b'z').map(|b| {
        let c = b as char;
        format!("\"{}\":{}", c, fq.get(&c).copied().unwrap_or(0))
    }).collect();
    format!("{{\"result\":{{{}}}}}", pairs.join(","))
}
pub fn ngram_distinct(t: &str, size: &str) -> String {
    let n = iv(size).max(1) as usize;
    let chars: Vec<char> = t.chars().collect();
    let mut set: std::collections::HashSet<String> = std::collections::HashSet::new();
    if chars.len() >= n {
        for i in 0..=(chars.len() - n) {
            set.insert(chars[i..i + n].iter().collect::<String>());
        }
    }
    jn(set.len().to_string())
}

// ===================== 文本距离 =====================
pub fn levenshtein_dist(t: &str, a: &str) -> String {
    let a1: Vec<char> = t.chars().collect();
    let b1: Vec<char> = a.chars().collect();
    let n = a1.len();
    let m = b1.len();
    let mut prev: Vec<usize> = (0..=m).collect();
    for i in 1..=n {
        let mut cur = vec![0usize; m + 1];
        cur[0] = i;
        for j in 1..=m {
            let cost = if a1[i - 1] == b1[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    jn(prev[m].to_string())
}
pub fn hamming_dist(t: &str, a: &str) -> String {
    if t.chars().count() != a.chars().count() { return j("长度不同"); }
    let d = t.chars().zip(a.chars()).filter(|(x, y)| x != y).count();
    jn(d.to_string())
}
pub fn palin_check(t: &str) -> String {
    let clean: Vec<char> = t.chars().filter(|c| c.is_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect();
    let rev: Vec<char> = clean.iter().rev().cloned().collect();
    j(if clean == rev { "true" } else { "false" })
}
pub fn anagram_check(t: &str, a: &str) -> String {
    let mut s1: Vec<char> = t.chars().filter(|c| !c.is_whitespace()).collect();
    let mut s2: Vec<char> = a.chars().filter(|c| !c.is_whitespace()).collect();
    s1.sort_unstable();
    s2.sort_unstable();
    j(if s1 == s2 { "true" } else { "false" })
}
pub fn lcs_length(t: &str, a: &str) -> String {
    let a1: Vec<char> = t.chars().collect();
    let b1: Vec<char> = a.chars().collect();
    let n = a1.len();
    let m = b1.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            dp[i][j] = if a1[i - 1] == b1[j - 1] { dp[i - 1][j - 1] + 1 } else { dp[i - 1][j].max(dp[i][j - 1]) };
        }
    }
    jn(dp[n][m].to_string())
}

// ===================== 地理 =====================
pub fn geo_haversine(t: &str, a: &str, b: &str, c: &str) -> String {
    // t=lat1 lon1, a=lat2 lon2 —— 用 b/c 占位，实际从 t/a 内逗号取值；此处 t 作为 lat1,lon1；a 作为 lat2,lon2
    let _ = (b, c);
    let (lat1, lon1) = two(t);
    let (lat2, lon2) = two(a);
    const R: f64 = 6371.0;
    let dlat = rad(lat2 - lat1);
    let dlon = rad(lon2 - lon1);
    let h = (dlat / 2.0).sin().powi(2) + rad(lat1).cos() * rad(lat2).cos() * (dlon / 2.0).sin().powi(2);
    let d = 2.0 * R * h.sqrt().asin();
    jn(format!("{:.2}", d))
}
fn rad(d: f64) -> f64 {
    d.to_radians()
}
fn two(s: &str) -> (f64, f64) {
    let v: Vec<f64> = s.split([',', ' ']).filter_map(|x| x.trim().parse::<f64>().ok()).take(2).collect();
    if v.len() == 2 { (v[0], v[1]) } else { (0.0, 0.0) }
}

// ===================== 业务 / 换算 =====================
pub fn percent_change(t: &str, a: &str) -> String {
    let old: f64 = t.trim().parse().unwrap_or(0.0);
    let new: f64 = a.trim().parse().unwrap_or(0.0);
    if old == 0.0 { return j("旧值为0，无法计算"); }
    jn(format!("{:.2}", (new - old) / old * 100.0))
}
pub fn compound_growth(t: &str, a: &str, b: &str) -> String {
    let p: f64 = t.trim().parse().unwrap_or(0.0);
    let r: f64 = a.trim().parse().unwrap_or(0.0) / 100.0;
    let n: f64 = b.trim().parse().unwrap_or(0.0);
    jn(format!("{:.2}", p * (1.0 + r).powi(n as i32)))
}
pub fn discount_price(t: &str, a: &str) -> String {
    let p: f64 = t.trim().parse().unwrap_or(0.0);
    let d: f64 = a.trim().parse::<f64>().unwrap_or(0.0).min(100.0);
    jn(format!("{:.2}", p * (1.0 - d / 100.0)))
}
pub fn tip_split(t: &str, a: &str, b: &str) -> String {
    let total: f64 = t.trim().parse().unwrap_or(0.0);
    let tip: f64 = a.trim().parse::<f64>().unwrap_or(0.0).min(100.0);
let people: f64 = b.trim().parse::<f64>().unwrap_or(1.0).max(1.0);
    jn(format!("{:.2}", total * (1.0 + tip / 100.0) / people))
}
pub fn bmi_calc(t: &str, a: &str) -> String {
    let kg: f64 = t.trim().parse().unwrap_or(0.0);
    let m: f64 = a.trim().parse().unwrap_or(1.0);
    if m <= 0.0 { return j("身高无效"); }
    let bmi = kg / (m * m);
    let label = if bmi < 18.5 { "偏瘦" } else if bmi < 24.0 { "正常" } else if bmi < 28.0 { "偏胖" } else { "肥胖" };
    j(&format!("{:.1}({})", bmi, label))
}
pub fn kelvin_to_c(t: &str) -> String {
    let k: f64 = t.trim().parse().unwrap_or(0.0);
    jn(format!("{:.2}", k - 273.15))
}
pub fn c_to_kelvin(t: &str) -> String {
    let c: f64 = t.trim().parse().unwrap_or(0.0);
    jn(format!("{:.2}", c + 273.15))
}
pub fn kmh_to_mph(t: &str) -> String {
    let v: f64 = t.trim().parse().unwrap_or(0.0);
    jn(format!("{:.2}", v * 0.621371))
}
pub fn mph_to_kmh(t: &str) -> String {
    let v: f64 = t.trim().parse().unwrap_or(0.0);
    jn(format!("{:.2}", v / 0.621371))
}
pub fn loan_payment(t: &str, a: &str, b: &str) -> String {
    // t=本金, a=年利率%, b=期数(月)
    let p: f64 = t.trim().parse().unwrap_or(0.0);
    let mr: f64 = a.trim().parse().unwrap_or(0.0) / 100.0 / 12.0;
    let n: f64 = b.trim().parse::<f64>().unwrap_or(1.0).max(1.0);
    let pay = if mr == 0.0 { p / n } else { p * mr * (1.0 + mr).powi(n as i32) / ((1.0 + mr).powi(n as i32) - 1.0) };
    jn(format!("{:.2}", pay))
}

// ===================== 日期 / 时间 =====================
pub fn days_between_dates(t: &str, a: &str) -> String {
    let u1 = dt_unix(t);
    let u2 = dt_unix(a);
    match (u1, u2) {
        (Some(x), Some(y)) => jn(((y - x).abs() / 86400).to_string()),
        _ => j("无效日期"),
    }
}
fn dt_unix(s: &str) -> Option<i64> {
    let (y, m, d) = split_date(s)?;
    let mut days = 0i64;
    for yy in 1970..y { days += if leap(yy) { 366 } else { 365 }; }
    for mm in 1..m { days += if mm == 2 { if leap(y) { 29 } else { 28 } } else if [4, 6, 9, 11].contains(&mm) { 30 } else { 31 }; }
    days += (d - 1) as i64;
    Some(days * 86400)
}
fn split_date(s: &str) -> Option<(i64, i64, i64)> {
    let parts: Vec<&str> = s.trim().split('-').collect();
    if parts.len() != 3 { return None; }
    Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
}
fn leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
pub fn day_of_year(t: &str) -> String {
    let (y, m, d) = match split_date(t) { Some(x) => x, None => return j("无效日期") };
    let mut days = d;
    for mm in 1..m {
        days += if mm == 2 { if leap(y) { 29 } else { 28 } } else if [4, 6, 9, 11].contains(&mm) { 30 } else { 31 };
    }
    jn(days.to_string())
}
pub fn month_days_of(t: &str, a: &str) -> String {
    // t=年份, a=月份
    let y = iv(t);
    let m = iv(a).clamp(1, 12);
    let d = if m == 2 { if leap(y) { 29 } else { 28 } } else if [4, 6, 9, 11].contains(&m) { 30 } else { 31 };
    jn(d.to_string())
}
pub fn weekday_of(t: &str) -> String {
    // Zeller 公式，输入 yyyy-mm-dd
    let (y, m, d) = match split_date(t) { Some(x) => x, None => return j("无效日期") };
    let (mut y, m, d) = if m < 3 { (y - 1, m + 12, d) } else { (y, m, d) };
    let y0 = y - (14 - m) / 12;
    let x = y0 + y0 / 4 - y0 / 100 + y0 / 400;
    let m0 = m + 12 * ((14 - m) / 12) - 2;
    let w = (d + x + (31 * m0) / 12) % 7;
    let names = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
    j(names[w as usize])
}

// ===================== CSV / 分隔 =====================
pub fn csv_row_count(t: &str) -> String {
    let lines: Vec<&str> = t.lines().filter(|l| !l.trim().is_empty()).collect();
    let n = if lines.first().map_or(false, |l| l.contains(',')) { lines.len() } else { lines.len() };
    jn(n.to_string())
}
pub fn csv_first_cols(t: &str) -> String {
    let first = t.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let n = first.split(',').count();
    jn(n.to_string())
}
pub fn delim_repeat(t: &str, delim: &str) -> String {
    jn(t.matches(delim).count().to_string())
}
pub fn stat_geomean(t: &str) -> String {
    let v = nums(t);
    if v.is_empty() { return jn("0".to_string()); }
    let p: f64 = v.iter().filter(|&&x| x > 0.0).map(|&x| x.ln()).sum();
    let n = v.iter().filter(|&&x| x > 0.0).count();
    if n == 0 { return j("无非正数"); }
    jn(format!("{:.4}", (p / n as f64).exp()))
}
pub fn value_size(t: &str) -> String {
    jn(nums(t).len().to_string())
}
pub fn stat_peak_to_avg(t: &str) -> String {
    let v = nums(t);
    if v.is_empty() { return jn("0".to_string()); }
    let max = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let avg = v.iter().sum::<f64>() / v.len() as f64;
    if avg == 0.0 { return j("均值为0"); }
    jn(format!("{:.2}", max / avg))
}
pub fn dist_manhattan(t: &str, a: &str) -> String {
    let v1 = nums(t);
    let v2 = nums(a);
    if v1.len() != v2.len() { return j("维度不同"); }
    let d: f64 = v1.iter().zip(v2.iter()).map(|(x, y)| (x - y).abs()).sum();
    jn(format!("{:.2}", d))
}