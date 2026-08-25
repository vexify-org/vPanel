//! 内置工具 · 数学与单位换算（`ops4`）。
//! 全部为纯函数，随调随算、无状态，常驻内存不变。命名不与 ops/ops2/ops3 重复。

use crate::json;

fn j(s: &str) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(s))
}
fn jn(n: String) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(&n))
}
fn num(t: &str) -> f64 {
    t.trim().parse::<f64>().unwrap_or_default()
}

// ---- 类型判定 ----
pub fn is_whole(t: &str) -> String { j(if num(t).fract() == 0.0 { "true" } else { "false" }) }
pub fn is_positive(t: &str) -> String { j(if num(t) > 0.0 { "true" } else { "false" }) }
pub fn is_negative(t: &str) -> String { j(if num(t) < 0.0 { "true" } else { "false" }) }
pub fn is_even(t: &str) -> String { let n = num(t); j(if n.fract()==0.0 && (n as i64)%2==0 { "true" } else { "false" }) }
pub fn is_odd(t: &str) -> String { let n = num(t); j(if n.fract()==0.0 && (n as i64)%2!=0 { "true" } else { "false" }) }
pub fn is_integer_range(t: &str) -> String { j(if num(t) >= -2147483648.0 && num(t) <= 2147483647.0 { "true" } else { "false" }) }

// ---- 取整 / 符号 ----
pub fn floor_num(t: &str) -> String { jn(num(t).floor().to_string()) }
pub fn ceil_num(t: &str) -> String { jn(num(t).ceil().to_string()) }
pub fn trunc_num(t: &str) -> String { jn(num(t).trunc().to_string()) }
pub fn round_num(t: &str) -> String { jn(num(t).round().to_string()) }
pub fn abs_num(t: &str) -> String { jn(num(t).abs().to_string()) }
pub fn sign_num(t: &str) -> String { let n=num(t); jn((if n>0.0{"1"}else if n<0.0{"-1"}else{"0"}).to_string()) }
pub fn negate_num(t: &str) -> String { jn((-num(t)).to_string()) }

// ---- 基础运算（多参）----
pub fn add_three(a: &str, b: &str, c: &str) -> String { jn((num(a)+num(b)+num(c)).to_string()) }
pub fn mul_three(a: &str, b: &str, c: &str) -> String { jn((num(a)*num(b)*num(c)).to_string()) }
pub fn sub2(a: &str, b: &str) -> String { jn((num(a)-num(b)).to_string()) }
pub fn div2(a: &str, b: &str) -> String { let d=num(b); if d==0.0 { j("除数不能为0") } else { jn((num(a)/d).to_string()) } }
pub fn rem2(a: &str, b: &str) -> String { let d=num(b) as i64; if d==0 { j("除数不能为0") } else { jn(((num(a) as i64)%d).to_string()) } }
pub fn avg2(a: &str, b: &str) -> String { jn(((num(a)+num(b))/2.0).to_string()) }
pub fn avg3(a: &str, b: &str, c: &str) -> String { jn(((num(a)+num(b)+num(c))/3.0).to_string()) }

// ---- 幂 / 根 / 对数 ----
pub fn square_num(t: &str) -> String { jn((num(t)*num(t)).to_string()) }
pub fn cube_num(t: &str) -> String { let n=num(t); jn((n*n*n).to_string()) }
pub fn sqrt_num(t: &str) -> String { let n=num(t); if n<0.0 { j("负数不能开平方") } else { jn(n.sqrt().to_string()) } }
pub fn inv_num(t: &str) -> String { let n=num(t); if n==0.0 { j("0 无倒数") } else { jn((1.0/n).to_string()) } }
pub fn pow2(t: &str) -> String { let n=num(t) as i32; if n<0||n>62 { j("指数越界0-62") } else { jn((2u64.pow(n as u32)).to_string()) } }
pub fn pow10x(t: &str) -> String { let n=num(t) as i32; if n<0||n>18 { j("指数越界0-18") } else { jn((10u64.pow(n as u32)).to_string()) } }
pub fn log10_num(t: &str) -> String { let n=num(t); if n<=0.0 { j("需正数") } else { jn(n.log10().to_string()) } }
pub fn log2_num(t: &str) -> String { let n=num(t); if n<=0.0 { j("需正数") } else { jn(n.log2().to_string()) } }
pub fn ln_num(t: &str) -> String { let n=num(t); if n<=0.0 { j("需正数") } else { jn(n.ln().to_string()) } }
pub fn exp_num(t: &str) -> String { jn(num(t).exp().to_string()) }

// ---- 三角函数（角度制，把度转弧度）----
fn d2r(d: f64) -> f64 { d.to_radians() }
pub fn sin_deg(t: &str) -> String { jn(d2r(num(t)).sin().to_string()) }
pub fn cos_deg(t: &str) -> String { jn(d2r(num(t)).cos().to_string()) }
pub fn tan_deg(t: &str) -> String { jn(d2r(num(t)).tan().to_string()) }
pub fn rad2deg(t: &str) -> String { jn(num(t).to_degrees().to_string()) }
pub fn deg2rad(t: &str) -> String { jn(num(t).to_radians().to_string()) }

// ---- 数列 / 组合 ----
pub fn fib_num(t: &str) -> String {
    let n = num(t) as u64;
    if n > 92 { return j("n 过大(<=92)"); }
    let (mut a1, mut b1) = (0u64, 1u64);
    for _ in 0..n { let t2 = a1 + b1; a1 = b1; b1 = t2; }
    jn(a1.to_string())
}
pub fn triangular_num(t: &str) -> String { let n=num(t) as i64; if n<0 { j("需非负") } else { jn((n*(n+1)/2).to_string()) } }
pub fn digit_sum(t: &str) -> String {
    let s = num(t).abs().to_string();
    let sum: u64 = s.chars().filter_map(|c| c.to_digit(10)).map(|d| d as u64).sum();
    jn(sum.to_string())
}
pub fn digit_count(t: &str) -> String { jn(num(t).abs().to_string().trim_end_matches(".0").len().to_string()) }
pub fn collatz_steps(t: &str) -> String {
    let mut n = num(t) as i64; if n <= 0 { return j("需正整数"); }
    let mut s = 0u32;
    while n != 1 && s < 100000 { if n % 2 == 0 { n /= 2 } else { n = 3 * n + 1 } s += 1; }
    jn(s.to_string())
}
pub fn is_prime2(t: &str) -> String {
    let n = num(t) as i64; if n < 2 { return j("false"); }
    let mut d = 2i64; while d * d <= n { if n % d == 0 { return j("false"); } d += 1; }
    j("true")
}
pub fn next_prime(t: &str) -> String {
    let mut n = (num(t) as i64).max(2); loop { let mut p = true; let mut d = 2i64; while d*d <= n && p { if n%d==0 { p=false } d+=1; } if p { return jn(n.to_string()); } n+=1; }
}

// ---- 几何 ----
pub fn circle_area(t: &str) -> String { let r=num(t); jn((r*r*std::f64::consts::PI).to_string()) }
pub fn circle_circumference(t: &str) -> String { jn((2.0*std::f64::consts::PI*num(t)).to_string()) }
pub fn sphere_volume(t: &str) -> String { let r=num(t); jn((4.0/3.0*std::f64::consts::PI*r*r*r).to_string()) }
pub fn pythagoras(a: &str, b: &str) -> String { jn((num(a)*num(a)+num(b)*num(b)).sqrt().to_string()) }
pub fn rect_perimeter(a: &str, b: &str) -> String { jn((2.0*(num(a)+num(b))).to_string()) }
pub fn rect_area(a: &str, b: &str) -> String { jn((num(a)*num(b)).to_string()) }

// ---- 温度换算 ----
pub fn c2f(t: &str) -> String { jn((num(t)*9.0/5.0+32.0).to_string()) }
pub fn f2c(t: &str) -> String { jn(((num(t)-32.0)*5.0/9.0).to_string()) }
pub fn c2k(t: &str) -> String { jn((num(t)+273.15).to_string()) }
pub fn k2c(t: &str) -> String { jn((num(t)-273.15).to_string()) }

// ---- 单位换算 ----
pub fn bytes_to_kb(t: &str) -> String { jn((num(t)/1024.0).to_string()) }
pub fn bytes_to_mb(t: &str) -> String { jn((num(t)/1048576.0).to_string()) }
pub fn bytes_to_gb(t: &str) -> String { jn((num(t)/1073741824.0).to_string()) }
pub fn gb_to_bytes(t: &str) -> String { jn((num(t)*1073741824.0).to_string()) }
pub fn mb_to_kb(t: &str) -> String { jn((num(t)*1024.0).to_string()) }
pub fn mbps_to_mbper_s(t: &str) -> String { jn((num(t)/8.0).to_string()) }
pub fn km_to_miles(t: &str) -> String { jn((num(t)*0.621371).to_string()) }
pub fn miles_to_km(t: &str) -> String { jn((num(t)/0.621371).to_string()) }
pub fn cm_to_m(t: &str) -> String { jn((num(t)/100.0).to_string()) }
pub fn m_to_km(t: &str) -> String { jn((num(t)/1000.0).to_string()) }
pub fn kg_to_lb(t: &str) -> String { jn((num(t)*2.20462).to_string()) }
pub fn hz_to_khz(t: &str) -> String { jn((num(t)/1000.0).to_string()) }
pub fn percent_of(part: &str, total: &str) -> String { let t=num(total); if t==0.0 { j("分母为0") } else { jn((num(part)/t*100.0).to_string()) } }

// ---- 随机 / 组合 ----
pub fn random_range(t: &str) -> String {
    let n = num(t) as u64; if n <= 0 { return j("需>0"); }
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();
    jn(((seed as u64) % n).to_string())
}
pub fn digits_of_pi(t: &str) -> String {
    let n = (num(t) as usize).min(15);
    let pi = std::f64::consts::PI;
    jn(format!("{:.width$}", pi, width = n))
}