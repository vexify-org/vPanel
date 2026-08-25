//! 内置工具 · 编码 / 校验和 / 进制（`ops6`）。
//! 纯函数、无状态，纯 std 手写实现，不引入外部 crate。每个工具功能独立。

use crate::json;

fn j(s: &str) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(s))
}
fn jn(n: String) -> String {
    format!("{{\"result\":\"{}\"}}", json::jesc(&n))
}

const D: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

fn rd(s: &str, base: u64) -> Option<u128> {
    let mut n: u128 = 0;
    for c in s.trim().to_ascii_lowercase().chars() {
        let d = c.to_digit(36).unwrap_or(0) as u128;
        if (d as u64) >= base { return None; }
        n = n * base as u128 + d;
    }
    Some(n)
}
fn wr(mut n: u128, base: u64) -> String {
    if n == 0 { return "0".to_string(); }
    let mut out = String::new();
    while n > 0 {
        out.push(D[(n % base as u128) as usize] as char);
        n /= base as u128;
    }
    out.chars().rev().collect()
}

// ---- 进制换算 ----
pub fn dec2bin(t: &str) -> String { match rd(t, 10) { Some(n) => jn(wr(n, 2)), None => j("无效十进制") } }
pub fn dec2oct(t: &str) -> String { match rd(t, 10) { Some(n) => jn(wr(n, 8)), None => j("无效十进制") } }
pub fn dec2hex(t: &str) -> String { match rd(t, 10) { Some(n) => jn(wr(n, 16)), None => j("无效十进制") } }
pub fn dec2hexu(t: &str) -> String { match rd(t, 10) { Some(n) => jn(wr(n, 16).to_uppercase()), None => j("无效十进制") } }
pub fn bin2dec(t: &str) -> String { match rd(t, 2) { Some(n) => jn(wr(n, 10)), None => j("无效二进制") } }
pub fn oct2dec(t: &str) -> String { match rd(t, 8) { Some(n) => jn(wr(n, 10)), None => j("无效八进制") } }
pub fn hex2dec(t: &str) -> String { match rd(t, 16) { Some(n) => jn(wr(n, 10)), None => j("无效十六进制") } }
pub fn bin2oct(t: &str) -> String { match rd(t, 2) { Some(n) => jn(wr(n, 8)), None => j("无效二进制") } }
pub fn bin2hex(t: &str) -> String { match rd(t, 2) { Some(n) => jn(wr(n, 16)), None => j("无效二进制") } }
pub fn oct2bin(t: &str) -> String { match rd(t, 8) { Some(n) => jn(wr(n, 2)), None => j("无效八进制") } }
pub fn oct2hex(t: &str) -> String { match rd(t, 8) { Some(n) => jn(wr(n, 16)), None => j("无效八进制") } }
pub fn hex2bin(t: &str) -> String { match rd(t, 16) { Some(n) => jn(wr(n, 2)), None => j("无效十六进制") } }
pub fn dec2base(t: &str, base: &str) -> String {
    let base = rd(base, 10).unwrap_or(0) as u64;
    if base < 2 || base > 36 { return j("基数需2-36"); }
    match rd(t, 10) { Some(n) => jn(wr(n, base)), None => j("无效十进制") }
}
pub fn base2dec(t: &str, base: &str) -> String {
    let base = rd(base, 10).unwrap_or(0) as u64;
    if base < 2 || base > 36 { return j("基数需2-36"); }
    match rd(t, base) { Some(n) => jn(wr(n, 10)), None => j("无效数字") }
}

// ---- 罗马数字 ----
pub fn dec2roman(t: &str) -> String {
    let mut n = rd(t, 10).unwrap_or(0) as i64;
    if n <= 0 || n >= 4000 { return j("范围 1-3999"); }
    let table = [(1000,"M"),(900,"CM"),(500,"D"),(400,"CD"),(100,"C"),(90,"XC"),(50,"L"),(40,"XL"),(10,"X"),(9,"IX"),(5,"V"),(4,"IV"),(1,"I")];
    let mut s = String::new();
    for &(v, rsym) in &table { while n >= v { s.push_str(rsym); n -= v; } }
    j(&s)
}
pub fn roman2dec(t: &str) -> String {
    let t = t.trim().to_uppercase();
    let mut total = 0i64; let mut prev = 0i64;
    for c in t.chars().rev() {
        let v = match c { 'I'=>1,'V'=>5,'X'=>10,'L'=>50,'C'=>100,'D'=>500,'M'=>1000,_=>return j("无效字符") };
        if v < prev { total -= v; } else { total += v; }
        prev = v;
    }
    jn(total.to_string())
}

// ---- 校验和 / 哈希 ----
pub fn sum_bytes(t: &str) -> String {
    let s: u64 = t.as_bytes().iter().map(|&b| b as u64).sum();
    jn(s.to_string())
}
pub fn xor_checksum(t: &str) -> String {
    let x = t.as_bytes().iter().fold(0u8, |a, &b| a ^ b);
    jn(x.to_string())
}
fn djb2(data: &[u8]) -> u64 { let mut h: u64 = 5381; for &b in data { h = h.wrapping_mul(33) ^ b as u64; } h }
pub fn djb2_hash(t: &str) -> String { jn(djb2(t.as_bytes()).to_string()) }
pub fn djb2_hash_hex(t: &str) -> String { jn(format!("{:x}", djb2(t.as_bytes()))) }
fn sdbm(data: &[u8]) -> u64 { let mut h: u64 = 0; for &b in data { h = b as u64 + (h << 6) + (h << 16) - h; } h }
pub fn sdbm_hash(t: &str) -> String { jn(sdbm(t.as_bytes()).to_string()) }
fn fnv1a32(data: &[u8]) -> u32 { let mut h: u32 = 2166136261; for &b in data { h ^= b as u32; h = h.wrapping_mul(16777619); } h }
fn fnv1a64(data: &[u8]) -> u64 { let mut h: u64 = 14695981039346656037; for &b in data { h ^= b as u64; h = h.wrapping_mul(1099511628211); } h }
pub fn fnv1a32_hash(t: &str) -> String { jn(format!("{:08x}", fnv1a32(t.as_bytes()))) }
pub fn fnv1a64_hash(t: &str) -> String { jn(format!("{:016x}", fnv1a64(t.as_bytes()))) }
fn adler32(data: &[u8]) -> u32 { let mut a = 1u32; let mut b = 0u32; for &x in data { a = (a + x as u32) % 65521; b = (b + a) % 65521; } (b << 16) | a }
pub fn adler32_cksum(t: &str) -> String { jn(format!("{:08x}", adler32(t.as_bytes()))) }
fn crc32(data: &[u8]) -> u32 { let mut c: u32 = 0xFFFFFFFF; for &b in data { c ^= b as u32; for _ in 0..8 { c = if c & 1 != 0 { (c >> 1) ^ 0xEDB88320 } else { c >> 1 }; } } c ^ 0xFFFFFFFF }
pub fn crc32_cksum(t: &str) -> String { jn(format!("{:08x}", crc32(t.as_bytes()))) }

// ---- 位运算统计 ----
pub fn hamming_weight(t: &str) -> String {
    jn(t.chars().filter(|&c| c == '1').count().to_string())
}
pub fn count_ones(t: &str) -> String {
    match rd(t, 10) { Some(n) => jn((n as u128).count_ones().to_string()), None => j("无效整数") }
}
pub fn parity_bit(t: &str) -> String {
    match rd(t, 10) { Some(n) => jn(((n as u128).count_ones() % 2).to_string()), None => j("无效整数") }
}
pub fn bit_length(t: &str) -> String {
    match rd(t, 10) { Some(n) => { let n = n as u128; jn((if n == 0 { 1 } else { (128 - n.leading_zeros()) as u32 }).to_string()) }, None => j("无效整数") }
}
pub fn is_power_of_two(t: &str) -> String {
    match rd(t, 10) { Some(n) => { let n = n as u128; j(if n != 0 && n & (n - 1) == 0 { "true" } else { "false" }) }, None => j("无效整数") }
}

// ---- 十六进制 / UTF-8 ----
fn hex_encode(data: &[u8]) -> String { data.iter().map(|b| format!("{:02x}", b)).collect() }
pub fn utf8_hex(t: &str) -> String { jn(hex_encode(t.as_bytes())) }
pub fn hex_decode_text(t: &str) -> String {
    let s: Vec<char> = t.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() % 2 != 0 { return j("长度需为偶数"); }
    let mut out = Vec::new();
    for i in (0..s.len()).step_by(2) {
        let hi = s[i].to_digit(16).unwrap_or(u32::MAX) as u32;
        let lo = s[i+1].to_digit(16).unwrap_or(u32::MAX) as u32;
        if hi > 15 || lo > 15 { return j("含非十六进制字符"); }
        out.push(((hi << 4) | lo) as u8);
    }
    j(&String::from_utf8_lossy(&out))
}
pub fn char_code(t: &str) -> String {
    match t.chars().next() { Some(c) => jn((c as u32).to_string()), None => j("空串") }
}
pub fn char_code_hex(t: &str) -> String {
    match t.chars().next() { Some(c) => jn(format!("{:x}", c as u32)), None => j("空串") }
}
pub fn utf8_len(t: &str) -> String { jn(t.as_bytes().len().to_string()) }

// ---- Base64 ----
const T64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
fn b64_encode(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(T64[(b0 >> 2) as usize] as char);
        out.push(T64[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 { out.push(T64[(((b1 & 15) << 2) | (b2 >> 6)) as usize] as char); } else { out.push('='); }
        if chunk.len() > 2 { out.push(T64[(b2 & 63) as usize] as char); } else { out.push('='); }
    }
    out
}
fn b64val(b: u8) -> Option<u32> {
    match b {
        b'A'..=b'Z' => Some((b - b'A') as u32),
        b'a'..=b'z' => Some((b - b'a') as u32 + 26),
        b'0'..=b'9' => Some((b - b'0') as u32 + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}
fn b64_decode(s: &str) -> String {
    let mut out = Vec::new();
    let mut buf: u32 = 0; let mut bits: u32 = 0;
    for &b in s.as_bytes() {
        if b == b'=' { break; }
        let Some(v) = b64val(b) else { continue };
        buf = (buf << 6) | v; bits += 6;
        if bits >= 8 { bits -= 8; out.push((buf >> bits) as u8); }
    }
    String::from_utf8_lossy(&out).into_owned()
}
pub fn b64_encode_text(t: &str) -> String { jn(b64_encode(t.as_bytes())) }
pub fn b64_decode_text(t: &str) -> String { j(&b64_decode(t)) }
pub fn b64url_encode_text(t: &str) -> String {
    let mut s = b64_encode(t.as_bytes());
    s = s.replace('+', "-").replace('/', "_");
    while s.ends_with('=') { s.pop(); }
    jn(s)
}
pub fn b64url_decode_text(t: &str) -> String { j(&b64_decode(t)) }

// ---- 简易加密变换 ----
pub fn rot13(t: &str) -> String {
    j(&t.chars().map(|c| {
        if c.is_ascii_uppercase() { (((c as u8 - b'A') + 13) % 26 + b'A') as char }
        else if c.is_ascii_lowercase() { (((c as u8 - b'a') + 13) % 26 + b'a') as char }
        else { c }
    }).collect::<String>())
}
pub fn caesar_shift(t: &str, shift: &str) -> String {
    let k = rd(shift, 10).unwrap_or(0) as i32 % 26;
    let out: String = t.chars().map(|c| {
        if c.is_ascii_uppercase() {
            let b = ((c as i32 - 'A' as i32 + k).rem_euclid(26)) as u8 + b'A';
            b as char
        } else if c.is_ascii_lowercase() {
            let b = ((c as i32 - 'a' as i32 + k).rem_euclid(26)) as u8 + b'a';
            b as char
        } else { c }
    }).collect();
    j(&out)
}
pub fn xor_cipher(t: &str, key: &str) -> String {
    let kb = key.as_bytes();
    if kb.is_empty() { return j(t); }
    let out: Vec<u8> = t.as_bytes().iter().enumerate().map(|(i, &b)| b ^ kb[i % kb.len()]).collect();
    j(&String::from_utf8_lossy(&out))
}
pub fn swap_bytes(t: &str) -> String {
    let b = t.as_bytes();
    let s: Vec<u8> = (0..b.len()).map(|i| b[b.len() - 1 - i]).collect();
    jn(hex_encode(&s))
}