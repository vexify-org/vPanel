//! 极小的 JSON / 表单工具，刻意不引入 serde_json 以压低内存与二进制体积。
//! 只在拼装 API 响应、解析简单 POST 表单时使用。

/// JSON 字符串转义（同时兼容 HTML 常见字符）。
pub fn jesc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// 百分比形式的 key=value 解码（含 + 表示空格）。
fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                    out.push((h << 4) | l);
                    i += 3;
                    continue;
                }
                out.push(b[i]);
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// 解析 application/x-www-form-urlencoded 请求体。
pub fn parse_form(body: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(body);
    text.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (pct_decode(k), pct_decode(v)),
            None => (pct_decode(p), String::new()),
        })
        .collect()
}

/// 从表单中取指定字段的值。
pub fn form_get<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// 从 JSON 对象文本中读取一个标量字段（字符串去引号、bool/数字为原样文本）。
/// 找不到或类型不符返回 None。极小实现，够 auth 端点用即可。
pub fn json_field(body: &[u8], key: &str) -> Option<String> {
    let text = String::from_utf8_lossy(body);
    let needle = format!("\"{}\"", key);
    let pos = text.find(&needle)?;
    let after = text[pos + needle.len()..].trim_start();
    let rest = after.strip_prefix(':')?.trim_start();
    let mut chars = rest.chars();
    let first = chars.next()?;
    match first {
        '"' => {
            let mut s = String::new();
            while let Some(c) = chars.next() {
                match c {
                    '"' => break,
                    '\\' => {
                        if let Some(e) = chars.next() {
                            s.push(match e {
                                'n' => '\n',
                                'r' => '\r',
                                't' => '\t',
                                other => other,
                            });
                        }
                    }
                    c => s.push(c),
                }
            }
            Some(s)
        }
        't' => Some("true".into()),
        'f' => Some("false".into()),
        c if c.is_ascii_digit() || c == '-' => {
            let mut s = String::new();
            s.push(c);
            for c in chars {
                if c == ',' || c == '}' || c == ']' || c.is_whitespace() {
                    break;
                }
                s.push(c);
            }
            Some(s)
        }
        _ => None,
    }
}

/// JSON 布尔字段为真判断。
pub fn json_bool(body: &[u8], key: &str) -> bool {
    matches!(json_field(body, key).as_deref(), Some("true"))
}

/// 运行一条命令，返回其 stdout（超时 5s，失败返回 None）。用于读类操作。
pub fn run_out(cmd: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jesc_escapes_specials() {
        assert_eq!(jesc("a\"b"), "a\\\"b");
        assert_eq!(jesc("a\\b"), "a\\\\b");
        assert_eq!(jesc("a\nb"), "a\\nb");
        assert_eq!(jesc("a\r\tb"), "a\\r\\tb");
    }

    #[test]
    fn jesc_keeps_plain_utf8() {
        assert_eq!(jesc("hello 世界"), "hello 世界");
        assert_eq!(jesc(""), "");
    }

    #[test]
    fn jesc_escapes_control_bytes() {
        assert_eq!(jesc("\u{01}"), "\\u0001");
    }

    #[test]
    fn parse_form_decodes_percent_and_plus() {
        let f = parse_form(b"a=hello+world&b=%E4%B8%AD%E6%96%87&c");
        assert_eq!(form_get(&f, "a"), Some("hello world"));
        assert_eq!(form_get(&f, "b"), Some("中文"));
        // 无 '=' 的键 → 空值
        assert_eq!(form_get(&f, "c"), Some(""));
    }

    #[test]
    fn parse_form_empty_body() {
        assert!(parse_form(b"").is_empty());
        assert!(parse_form(b"&").is_empty());
    }

    #[test]
    fn parse_form_malformed_percent_is_literal() {
        // 不完整 / 非法百分号按原样保留
        let f = parse_form(b"a=100%");
        assert_eq!(form_get(&f, "a"), Some("100%"));
    }

    #[test]
    fn json_field_reads_string_number_bool() {
        assert_eq!(json_field(b"{\"k\":\"v\"}", "k").as_deref(), Some("v"));
        assert_eq!(json_field(b"{\"k\": 42}", "k").as_deref(), Some("42"));
        assert_eq!(json_field(b"{\"k\":true}", "k").as_deref(), Some("true"));
        assert_eq!(json_field(b"{\"k\":false}", "k").as_deref(), Some("false"));
    }

    #[test]
    fn json_field_unescapes() {
        assert_eq!(json_field(b"{\"k\":\"a\\nb\"}", "k").as_deref(), Some("a\nb"));
    }

    #[test]
    fn json_field_missing_is_none() {
        assert_eq!(json_field(b"{\"x\":1}", "k"), None);
        assert_eq!(json_field(b"not json", "k"), None);
    }

    #[test]
    fn json_bool_matches_true_only() {
        assert!(json_bool(b"{\"k\":true}", "k"));
        assert!(!json_bool(b"{\"k\":false}", "k"));
        // 非 "true" 的字符串算作 false
        assert!(!json_bool(b"{\"k\":\"yes\"}", "k"));
        assert!(!json_bool(b"{}", "k"));
    }
}