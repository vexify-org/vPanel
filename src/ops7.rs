//! 内置工具 · 时间与日期（`ops7`）。
//! 纯 std 手写儒略日/公历换算，不依赖 chrono。每个工具功能独立。

use crate::json;
use std::time::{SystemTime, UNIX_EPOCH};

fn j(s: &str) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(s))
}
fn jn(n: String) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(&n))
}
fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// 公历 → 1970-01-01 起的天数（Howard Hinnant 逆算法）。
fn civil_to_days(y: i64, m: i64, d: i64) -> i64 {
    let yy = if m <= 2 { y - 1 } else { y };
    let era = if yy >= 0 { yy } else { yy - 399 } / 400;
    let yoe = yy - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}
/// 天数 → 公历分量 (y,m,d,h,mi,s)。
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}
fn epoch_parts(secs: i64) -> (i64, i64, i64, i64, i64, i64) {
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    (y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}
/// 校验 (y,m,d) 是否合法公历日期。
fn valid_ymd(y: i64, m: i64, d: i64) -> bool {
    if m < 1 || m > 12 || d < 1 { return false; }
    d <= days_in_month(y, m)
}
fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 29 } else { 28 },
        _ => 0,
    }
}
/// 解析 "YYYY-MM-DD[ HH:MM:SS]"。
fn parse_dt(s: &str) -> Option<(i64, i64, i64, i64, i64, i64)> {
    let mut it = s.trim().split(|c: char| c == '-' || c == ':' || c == ' ' || c == 'T');
    let y: i64 = it.next()?.trim().parse().ok()?;
    let m: i64 = it.next()?.trim().parse().ok()?;
    let d: i64 = it.next()?.trim().parse().ok()?;
    let h: i64 = it.next().map(|x| x.trim().parse().unwrap_or(0)).unwrap_or(0);
    let mi: i64 = it.next().map(|x| x.trim().parse().unwrap_or(0)).unwrap_or(0);
    let s: i64 = it.next().map(|x| x.trim().parse().unwrap_or(0)).unwrap_or(0);
    Some((y, m, d, h, mi, s))
}
fn dt_to_unix(s: &str) -> Option<i64> {
    let (y, m, d, h, mi, sec) = parse_dt(s)?;
    if !valid_ymd(y, m, d) || h > 23 || mi > 59 || sec > 59 { return None; }
    Some(civil_to_days(y, m, d) * 86400 + h * 3600 + mi * 60 + sec)
}
fn dow_num(days: i64) -> i64 {
    ((days + 3).rem_euclid(7)) + 1 // Mon=1..Sun=7
}
fn dow_name(n: i64) -> &'static str {
    match n { 1 => "周一", 2 => "周二", 3 => "周三", 4 => "周四", 5 => "周五", 6 => "周六", _ => "周日" }
}
fn day_of_year(y: i64, m: i64, d: i64) -> i64 {
    let mut total = 0i64;
    for mo in 1..m { total += days_in_month(y, mo); }
    total + d
}
const MONTH_CN: [&str; 12] = ["一月", "二月", "三月", "四月", "五月", "六月", "七月", "八月", "九月", "十月", "十一月", "十二月"];

// ---- 当前时间 ----
pub fn now_unix() -> String { jn(now().to_string()) }
pub fn now_millis() -> String {
    let m = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    jn(m.to_string())
}
pub fn now_date() -> String { let (y, m, d, _, _, _) = epoch_parts(now()); j(&format!("{:04}-{:02}-{:02}", y, m, d)) }
pub fn now_time() -> String { let (_, _, _, h, mi, s) = epoch_parts(now()); j(&format!("{:02}:{:02}:{:02}", h, mi, s)) }
pub fn now_datetime() -> String { let (y, m, d, h, mi, s) = epoch_parts(now()); j(&format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, h, mi, s)) }
pub fn now_weekday() -> String { let days = now().div_euclid(86400); j(dow_name(dow_num(days))) }
pub fn now_dow() -> String { let days = now().div_euclid(86400); jn(dow_num(days).to_string()) }
pub fn now_iso() -> String { let (y, m, d, h, mi, s) = epoch_parts(now()); j(&format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, h, mi, s)) }
pub fn now_uptime() -> String {
    let s = std::fs::read_to_string("/proc/uptime").ok().and_then(|x| x.split_whitespace().next().map(|v| v.parse::<f64>().unwrap_or(0.0))).unwrap_or(0.0) as i64;
    jn(s.to_string())
}

// ---- 时间戳取分量 ----
pub fn unix_to_date(t: &str) -> String { let (y, m, d, _, _, _) = epoch_parts(t.trim().parse().unwrap_or(0)); j(&format!("{:04}-{:02}-{:02}", y, m, d)) }
pub fn unix_to_time(t: &str) -> String { let (_, _, _, h, mi, s) = epoch_parts(t.trim().parse().unwrap_or(0)); j(&format!("{:02}:{:02}:{:02}", h, mi, s)) }
pub fn unix_to_datetime(t: &str) -> String { let p = epoch_parts(t.trim().parse().unwrap_or(0)); j(&format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", p.0, p.1, p.2, p.3, p.4, p.5)) }
pub fn unix_to_weekday(t: &str) -> String { let days = t.trim().parse::<i64>().unwrap_or(0).div_euclid(86400); j(dow_name(dow_num(days))) }
pub fn unix_year(t: &str) -> String { jn(epoch_parts(t.trim().parse().unwrap_or(0)).0.to_string()) }
pub fn unix_month(t: &str) -> String { jn(epoch_parts(t.trim().parse().unwrap_or(0)).1.to_string()) }
pub fn unix_day(t: &str) -> String { jn(epoch_parts(t.trim().parse().unwrap_or(0)).2.to_string()) }
pub fn unix_hour(t: &str) -> String { jn(epoch_parts(t.trim().parse().unwrap_or(0)).3.to_string()) }
pub fn unix_minute(t: &str) -> String { jn(epoch_parts(t.trim().parse().unwrap_or(0)).4.to_string()) }
pub fn unix_second(t: &str) -> String { jn(epoch_parts(t.trim().parse().unwrap_or(0)).5.to_string()) }
pub fn unix_doy(t: &str) -> String { let p = epoch_parts(t.trim().parse().unwrap_or(0)); jn(day_of_year(p.0, p.1, p.2).to_string()) }
pub fn unix_dow(t: &str) -> String { let days = t.trim().parse::<i64>().unwrap_or(0).div_euclid(86400); jn(dow_num(days).to_string()) }

// ---- 日期判定 / 计算 ----
pub fn is_leap_yr(t: &str) -> String {
    let y = t.trim().parse::<i64>().unwrap_or(0);
    j(if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { "true" } else { "false" })
}
pub fn days_in_m(t: &str) -> String {
    let mut it = t.trim().split('-');
    let y: i64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(2024);
    let m: i64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(1);
    if m < 1 || m > 12 { return j("月份1-12"); }
    jn(days_in_month(y, m).to_string())
}
pub fn days_in_yr(t: &str) -> String {
    let y = t.trim().parse::<i64>().unwrap_or(2024);
    jn((if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 366 } else { 365 }).to_string())
}
pub fn date_dow(t: &str) -> String {
    let (y, m, d, _, _, _) = parse_dt(t).unwrap_or((1970, 1, 1, 0, 0, 0));
    if !valid_ymd(y, m, d) { return j("无效日期"); }
    jn(dow_num(civil_to_days(y, m, d)).to_string())
}
pub fn date_weekday(t: &str) -> String {
    let (y, m, d, _, _, _) = parse_dt(t).unwrap_or((1970, 1, 1, 0, 0, 0));
    if !valid_ymd(y, m, d) { return j("无效日期"); }
    j(dow_name(dow_num(civil_to_days(y, m, d))))
}
pub fn date_doy(t: &str) -> String {
    let (y, m, d, _, _, _) = parse_dt(t).unwrap_or((1970, 1, 1, 0, 0, 0));
    if !valid_ymd(y, m, d) { return j("无效日期"); }
    jn(day_of_year(y, m, d).to_string())
}
pub fn is_weekend(t: &str) -> String {
    let (y, m, d, _, _, _) = parse_dt(t).unwrap_or((1970, 1, 1, 0, 0, 0));
    if !valid_ymd(y, m, d) { return j("无效日期"); }
    let n = dow_num(civil_to_days(y, m, d));
    j(if n >= 6 { "true" } else { "false" })
}
pub fn week_of_year(t: &str) -> String {
    let (y, m, d, _, _, _) = parse_dt(t).unwrap_or((1970, 1, 1, 0, 0, 0));
    if !valid_ymd(y, m, d) { return j("无效日期"); }
    jn(((day_of_year(y, m, d) - 1) / 7 + 1).to_string())
}
pub fn is_month_first(t: &str) -> String {
    let (_, _, d, _, _, _) = parse_dt(t).unwrap_or((1970, 1, 1, 0, 0, 0));
    j(if d == 1 { "true" } else { "false" })
}
pub fn is_month_last(t: &str) -> String {
    let (y, m, d, _, _, _) = parse_dt(t).unwrap_or((1970, 1, 1, 0, 0, 0));
    if !valid_ymd(y, m, d) { return j("无效日期"); }
    j(if d == days_in_month(y, m) { "true" } else { "false" })
}
pub fn month_name(t: &str) -> String {
    let n = t.trim().parse::<i64>().unwrap_or(0);
    if n < 1 || n > 12 { return j("月份1-12"); }
    j(MONTH_CN[(n - 1) as usize])
}
pub fn month_days_txt(t: &str) -> String {
    let n = t.trim().parse::<i64>().unwrap_or(0);
    jn(days_in_month(2024, n).to_string())
}

// ---- 日期运算（text=日期，a/b=增量）----
pub fn date_to_unix_stamp(t: &str) -> String {
    match dt_to_unix(t) { Some(v) => jn(v.to_string()), None => j("无效日期") }
}
pub fn add_days(t: &str, a: &str) -> String {
    let secs = match dt_to_unix(t) { Some(v) => v, None => return j("无效日期") };
    let step: i64 = a.trim().parse().unwrap_or(0);
    let (y, m, d, h, mi, s) = epoch_parts(secs + step * 86400);
    j(&format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, h, mi, s))
}
pub fn add_hours(t: &str, a: &str) -> String {
    let secs = match dt_to_unix(t) { Some(v) => v, None => return j("无效日期") };
    let step: i64 = a.trim().parse().unwrap_or(0);
    let (y, m, d, h, mi, s) = epoch_parts(secs + step * 3600);
    j(&format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, h, mi, s))
}
pub fn add_minutes(t: &str, a: &str) -> String {
    let secs = match dt_to_unix(t) { Some(v) => v, None => return j("无效日期") };
    let step: i64 = a.trim().parse().unwrap_or(0);
    let (y, m, d, h, mi, s) = epoch_parts(secs + step * 60);
    j(&format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, h, mi, s))
}
pub fn add_months(t: &str, a: &str) -> String {
    let (y, m, d, h, mi, s) = parse_dt(t).unwrap_or((1970, 1, 1, 0, 0, 0));
    if !valid_ymd(y, m, d) { return j("无效日期"); }
    let step: i64 = a.trim().parse().unwrap_or(0);
    let total = y * 12 + m - 1 + step;
    let mut ny = total.div_euclid(12);
    let mut nm = total.rem_euclid(12) + 1;
    let maxd = days_in_month(ny, nm);
    let nd = d.min(maxd);
    if ny < 1 { ny = 1; nm = 1; }
    j(&format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", ny, nm, nd, h, mi, s))
}
pub fn add_years(t: &str, a: &str) -> String {
    let (y, m, d, h, mi, s) = parse_dt(t).unwrap_or((1970, 1, 1, 0, 0, 0));
    if !valid_ymd(y, m, d) { return j("无效日期"); }
    let step: i64 = a.trim().parse().unwrap_or(0);
    let ny = (y + step).max(1);
    let nd = d.min(days_in_month(ny, m));
    j(&format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", ny, m, nd, h, mi, s))
}
pub fn date_diff_days(a: &str, b: &str) -> String {
    let (ua, ub) = match (dt_to_unix(a), dt_to_unix(b)) { (Some(x), Some(y)) => (x, y), _ => return j("无效日期") };
    jn(((ua - ub).abs() / 86400).to_string())
}
pub fn date_diff_seconds(a: &str, b: &str) -> String {
    let (ua, ub) = match (dt_to_unix(a), dt_to_unix(b)) { (Some(x), Some(y)) => (x, y), _ => return j("无效日期") };
    jn((ua - ub).abs().to_string())
}

// ---- 时间跨度格式化 ----
pub fn seconds_to_hms(t: &str) -> String {
    let mut s = t.trim().parse::<i64>().unwrap_or(0).max(0);
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    s = 0; let _ = s;
    j(&format!("{:02}:{:02}:{:02}", h, m, sec))
}
pub fn minutes_to_hms(t: &str) -> String {
    let mut s = t.trim().parse::<i64>().unwrap_or(0).max(0) * 60;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    s = 0; let _ = s;
    j(&format!("{:02}:{:02}:{:02}", h, m, sec))
}
pub fn hours_to_days(t: &str) -> String {
    let h = t.trim().parse::<i64>().unwrap_or(0).max(0);
    jn(format!("{}天{}小时", h / 24, h % 24))
}
pub fn seconds_to_human(t: &str) -> String {
    let mut s = t.trim().parse::<i64>().unwrap_or(0).max(0);
    let d = s / 86400; s %= 86400;
    let h = s / 3600; s %= 3600;
    let m = s / 60; s %= 60;
    let mut out = Vec::new();
    if d > 0 { out.push(format!("{}天", d)); }
    if h > 0 { out.push(format!("{}时", h)); }
    if m > 0 { out.push(format!("{}分", m)); }
    if s > 0 || out.is_empty() { out.push(format!("{}秒", s)); }
    j(&out.join(""))
}
pub fn ms_to_seconds(t: &str) -> String {
    jn(format!("{:.3}", t.trim().parse::<f64>().unwrap_or(0.0) / 1000.0))
}
pub fn days_to_years(t: &str) -> String {
    let d = t.trim().parse::<f64>().unwrap_or(0.0);
    jn(format!("{:.2}", d / 365.25))
}