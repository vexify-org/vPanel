//! 自研的「微脚本」解释器 —— vPanel 插件语言运行时。
//!
//! 刻意做成极小、精简化，避免引入 Lua 之类的重型解释器，以维持低内存预算。
//! 变量即字符串，`+` 做字符串拼接，支持行式语句与少量内置函数：
//!
//! ```text
//! now   = cmd("date \"+%F %T\"")      # 执行 shell，捕获单身输出
//! out   = "uptime: " + now            # 字符串拼接
//! json  = fetch("https://example/a")  # HTTP GET（走 curl，支持 TLS）
//! log(out)                            # 记入面板插件日志
//! ret(out)                            # 标记为脚本返回值
//! ```
//!
//! 边界越界、解析失败均返回 `Err`，由调用方（插件执行器）处理，不影响面板。
//!
//! 为避免把系统能力写死在本模块，`cmd` / `fetch` 通过 [`Builtin`] trait 注入。

use std::collections::HashMap;

/// 外部能力：命令执行与 HTTP 拉取，由宿主（plugins）提供实现。
pub trait Builtin {
    /// 执行 shell 命令并返回 stdout（末尾换行会被裁剪）。
    fn cmd(&self, shell: &str) -> String;
    /// HTTP GET，返回响应体文本或错误信息。`timeout` 为秒。
    fn fetch(&self, url: &str, timeout: u64) -> String;
}

/// 解释器实例。一次脚本运行创建一个，跑完即丢弃，内存立即释放。
pub struct Interp<'a> {
    vars: HashMap<String, String>,
    retval: Option<String>,
    logs: Vec<String>,
    builtin: &'a dyn Builtin,
}

impl<'a> Interp<'a> {
    pub fn new(builtin: &'a dyn Builtin) -> Self {
        Interp {
            vars: HashMap::new(),
            retval: None,
            logs: Vec::new(),
            builtin,
        }
    }

    /// 运行整个脚本。返回 `ret(...)` 标记的返回值；未标记则返回空白。
    pub fn run(&mut self, script: &str) -> Result<String, String> {
        for line in script.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            self.exec(strip_line_comment(line))?;
        }
        Ok(self.retval.clone().unwrap_or_default())
    }

    /// 累积的 `log(...)` 输出。
    pub fn logs(&self) -> &[String] {
        &self.logs
    }

    fn exec(&mut self, line: &str) -> Result<(), String> {
        // 赋值：`ident = expr`
        if let Some(eq) = line.find('=') {
            let head = line[..eq].trim();
            let rest = line[eq + 1..].trim();
            if is_ident(head) {
                let v = self.eval(rest)?;
                self.vars.insert(head.to_string(), v);
                return Ok(());
            }
        }
        // 其它：作为表达式求值（通常用于调用 log(...) 等）。
        let _ = self.eval(line)?;
        Ok(())
    }

    /// 求值一个表达式：允许 `term ('+' term)*`。
    fn eval(&mut self, s: &str) -> Result<String, String> {
        let toks = tokenize(s)?;
        let mut p = Par { toks, pos: 0 };
        let out = self.parse_expr(&mut p)?;
        if p.pos != p.toks.len() {
            return Err("表达式多余符号".into());
        }
        Ok(out)
    }

    /// 解析一列 `+` 连接的 term。
    fn parse_expr(&mut self, p: &mut Par) -> Result<String, String> {
        let mut out = self.term(p)?;
        while p.peek_is("__plus") {
            p.next();
            out.push_str(&self.term(p)?);
        }
        Ok(out)
    }

    /// 一个 term：字符串 / 数字 / 变量 / 函数调用。
    fn term(&mut self, p: &mut Par) -> Result<String, String> {
        let t = p.next().ok_or("表达式提前结束")?;
        match t {
            Tok::Str(s) => Ok(self.interp_vars(&s)),
            Tok::Ident(name) => {
                if p.peek_is("__lparen") {
                    p.next();
                    self.call(name, p)
                } else {
                    Ok(self.vars.get(&name).cloned().unwrap_or_default())
                }
            }
            Tok::Number(n) => Ok(n),
            _ => Err("无法解析的 term".into()),
        }
    }

    /// 函数调用 `name(arg, arg, ...)`。
    fn call(&mut self, name: String, p: &mut Par) -> Result<String, String> {
        let mut args: Vec<String> = Vec::new();
        while !p.peek_is("__rparen") {
            args.push(self.parse_expr(p)?);
            if p.peek_is("__comma") {
                p.next();
                continue;
            }
            break;
        }
        if !p.peek_is("__rparen") {
            return Err("函数参数括号未闭合".into());
        }
        p.next();
        let one = args.get(0).cloned().unwrap_or_default();
        let _two = args.get(1).cloned().unwrap_or_default();
        Ok(match name.as_str() {
            "cmd" => self.builtin.cmd(&one),
            "fetch" => {
                let timeout = args.get(1).and_then(|t| t.parse().ok()).unwrap_or(8);
                self.builtin.fetch(&one, timeout)
            }
            "ret" => {
                self.retval = Some(one);
                String::new()
            }
            "log" => {
                self.logs.push(one.clone());
                String::new()
            }
            "env" => std::env::var(&one).unwrap_or_default(),
            "var" => self.vars.get(&one).cloned().unwrap_or_default(),
            other => return Err(format!("未知函数: {}", other)),
        })
    }

    /// 把字符串字面量中的 `{ident}` 替换为变量值。
    fn interp_vars(&self, s: &str) -> String {
        let mut out = String::new();
        let mut rest = s;
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            if let Some(end) = after.find('}') {
                let name = &after[..end];
                let v = self.vars.get(name).cloned().unwrap_or_default();
                out.push_str(&v);
                rest = &after[end + 1..];
            } else {
                out.push('{');
                rest = after;
            }
        }
        out.push_str(rest);
        out
    }
}

/// 行内 `#` 注释（字符串内的 `#` 不处理，简化处理即可）。
fn strip_line_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) if line.as_bytes().get(i.wrapping_sub(1)) != Some(&b'"') => &line[..i],
        _ => line,
    }
}

fn is_ident(s: &str) -> bool {
    let mut it = s.chars();
    match it.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    it.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Str(String),
    Number(String),
    Ident(String),
    Sym(&'static str),
}

struct Par {
    toks: Vec<Tok>,
    pos: usize,
}

impl Par {
    fn peek_is(&self, sym: &str) -> bool {
        matches!(self.toks.get(self.pos), Some(Tok::Sym(s)) if *s == sym)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }
}

/// 把一行表达式切分为 token。
fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let b = s.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '"' => {
                // 字符串字面量：按字节收集，遇未转义双引号结束，最后 UTF-8 解码，
                // 避免逐字节转 char 破坏多字节中文。
                let mut out: Vec<u8> = Vec::new();
                let mut j = i + 1;
                while j < b.len() {
                    if b[j] == b'\\' && j + 1 < b.len() {
                        let nc = b[j + 1];
                        match nc {
                            b'n' => out.push(b'\n'),
                            b't' => out.push(b'\t'),
                            b'"' => out.push(b'"'),
                            b'\\' => out.push(b'\\'),
                            o => {
                                out.push(b'\\');
                                out.push(o);
                            }
                        }
                        j += 2;
                        continue;
                    }
                    if b[j] == b'"' {
                        break;
                    }
                    out.push(b[j]);
                    j += 1;
                }
                if j >= b.len() {
                    return Err("字符串未闭合".into());
                }
                toks.push(Tok::Str(String::from_utf8_lossy(&out).into_owned()));
                i = j + 1;
            }
            '(' => {
                toks.push(Tok::Sym("__lparen"));
                i += 1;
            }
            ')' => {
                toks.push(Tok::Sym("__rparen"));
                i += 1;
            }
            ',' => {
                toks.push(Tok::Sym("__comma"));
                i += 1;
            }
            '+' => {
                toks.push(Tok::Sym("__plus"));
                i += 1;
            }
            _ if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                i += 1;
                while i < b.len() && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                toks.push(Tok::Ident(s[start..i].to_string()));
            }
            _ if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < b.len() {
                    let d = b[i] as char;
                    if d.is_ascii_digit() || d == '.' || d == '-' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                toks.push(Tok::Number(s[start..i].to_string()));
            }
            _ => return Err(format!("无法识别的字符: {}", c)),
        }
    }
    Ok(toks)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct B;
    impl Builtin for B {
        fn cmd(&self, _s: &str) -> String {
            "OUT".to_string()
        }
        fn fetch(&self, _url: &str, _t: u64) -> String {
            "BODY".to_string()
        }
    }

    #[test]
    fn basic() {
        let mut i = Interp::new(&B);
        let r = i
            .run("x = cmd(\"a\")\nlog(x)\nret(\"v=\" + x)")
            .unwrap();
        assert_eq!(r, "v=OUT");
        assert_eq!(i.logs(), &["OUT"]);
    }

    #[test]
    fn interp_var() {
        let mut i = Interp::new(&B);
        i.run("a = \"hi\"\nret(\"{a}! {missing}\")").unwrap();
        assert_eq!(i.retval.as_deref(), Some("hi! "));
    }
}