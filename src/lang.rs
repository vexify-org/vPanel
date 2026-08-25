//! vPanel 插件微脚本语言 —— 自研解释器。
//!
//! 从"逐行"升级为"缩进块 + 控制流"的轻量语言，完全不引入重型运行时，
//! 每次脚本调用新建解释器、跑完即释放，保持低内存预算。
//!
//! 特性：
//! - 变量：字符串 / 数字 / 布尔
//! - 赋值、算术（+ - * / %）、字符串 `+` 拼接
//! - 比较（== != < <= > >=）与逻辑（and or not）
//! - 控制流：`if/else`、`for i in range(n)`、`while`、`break`/`continue`
//! - 块以缩进界定（块结束需回到父级缩进）
//! - 内置函数：cmd/fetch/ret/log/env/var/arg + 文本/数学/KV

use std::collections::HashMap;
use sha2::Sha256;
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// 外部能力接口（由 plugins 实现）
// ---------------------------------------------------------------------------

pub trait Builtin {
    fn cmd(&self, shell: &str) -> String;
    fn fetch(&self, url: &str, timeout: u64) -> String;
    /// HTTP POST，返回响应文本。
    fn post(&self, url: &str, body: &str) -> String;
    /// 探测 URL 的 HTTP 状态码（如 "200"），失败返回 "0"。
    fn http_status(&self, url: &str) -> String;
    fn kv_get(&self, key: &str) -> Option<String>;
    fn kv_set(&self, key: &str, val: &str) -> bool;
    fn arg(&self, name: &str) -> Option<String>;
    fn has_arg(&self, name: &str) -> bool;
    /// 读取文本文件，不存在返回空串。
    fn read_file(&self, path: &str) -> String;
    /// 覆盖写入文本文件，成功返回 true。
    fn write_file(&self, path: &str, content: &str) -> bool;
    /// 追加文本到文件。
    fn append_file(&self, path: &str, content: &str) -> bool;
    /// 列出目录：每行 `名称<tab>类型(d/f)<tab>大小`，非目录返回空。
    fn ls(&self, path: &str) -> String;
    /// 文件信息：`大小;<是否存在>;<是否目录>`。
    fn file_info(&self, path: &str) -> String;
    /// 解析主机 → 第一个 IP（失败返回空）。
    fn lookup_ip(&self, host: &str) -> String;
    /// 结束进程，成功返回 true。
    fn kill_pid(&self, pid: u32) -> bool;
    /// 计算文件的 SHA-1 摘要（小写 hex），不存在/失败返回 `-`。
    fn sha1(&self, path: &str) -> String;
    /// 将字符串做 URL 编码。
    fn urlenc(&self, s: &str) -> String;
}

// ---------------------------------------------------------------------------
// 值
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum V {
    S(String),
    N(f64),
    B(bool),
    /// 有序字符串列表，用于 for..in 迭代与批量处理。
    L(Vec<String>),
    Nil,
}

impl V {
    fn text(&self) -> String {
        match self {
            V::S(s) => s.clone(),
            V::N(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            V::B(b) => if *b { "true".into() } else { "false".into() },
            V::L(v) => v.join("\n"),
            V::Nil => String::new(),
        }
    }
    fn as_num(&self) -> f64 {
        match self {
            V::N(n) => *n,
            V::S(s) => s.trim().parse().unwrap_or(0.0),
            V::B(b) => if *b { 1.0 } else { 0.0 },
            V::L(v) => v.len() as f64,
            V::Nil => 0.0,
        }
    }
    fn truthy(&self) -> bool {
        match self {
            V::B(b) => *b,
            V::N(n) => *n != 0.0,
            V::S(s) => !s.is_empty(),
            V::L(v) => !v.is_empty(),
            V::Nil => false,
        }
    }
    fn is_list(&self) -> bool {
        matches!(self, V::L(_))
    }
    fn list(&self) -> Option<&[String]> {
        match self {
            V::L(v) => Some(v),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Token 与词法
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Op(String),
}

fn tokenize(line: &str) -> Result<Vec<Tok>, String> {
    let b = line.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '#' {
            break;
        }
        if c == '"' {
            let mut out = Vec::new();
            let mut j = i + 1;
            while j < b.len() {
                if b[j] == b'\\' && j + 1 < b.len() {
                    match b[j + 1] {
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
            continue;
        }
        if c.is_ascii_digit()
            || (c == '.' && i + 1 < b.len() && (b[i + 1] as char).is_ascii_digit())
        {
            let start = i;
            i += 1;
            while i < b.len() {
                let d = b[i] as char;
                if d.is_ascii_digit()
                    || d == '.'
                    || d == 'e'
                    || d == 'E'
                    || ((d == '+' || d == '-') && (b[i - 1] as char == 'e' || b[i - 1] as char == 'E'))
                {
                    i += 1;
                } else {
                    break;
                }
            }
            let num: f64 = line[start..i].parse().map_err(|_| "数字非法".to_string())?;
            toks.push(Tok::Num(num));
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while i < b.len() && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            toks.push(Tok::Ident(line[start..i].to_string()));
            continue;
        }
        let two = if i + 1 < b.len() { &line[i..i + 2] } else { "" };
        match two {
            "==" | "!=" | "<=" | ">=" | "&&" | "||" => {
                toks.push(Tok::Op(two.to_string()));
                i += 2;
                continue;
            }
            _ => {}
        }
        if "+-*/%<>=!(),".contains(c) {
            toks.push(Tok::Op(c.to_string()));
            i += 1;
            continue;
        }
        return Err(format!("无法识别的字符: {}", c));
    }
    Ok(toks)
}

// ---------------------------------------------------------------------------
// 语法树
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Expr {
    Lit(V),
    Var(String),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Bin(String, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone)]
enum Stmt {
    Assign(String, Expr),
    If { cond: Expr, then: Vec<Stmt>, els: Vec<Stmt> },
    For { var: String, n: Expr, body: Vec<Stmt> },
    While { cond: Expr, body: Vec<Stmt> },
    Break,
    Continue,
    Expr(Expr),
}

// ---------------------------------------------------------------------------
// 解析：先按缩进把行聚合成块，再做递归下降
// ---------------------------------------------------------------------------

struct Line {
    indent: usize,
    toks: Vec<Tok>,
}

fn parse(script: &str) -> Result<Vec<Stmt>, String> {
    let lines = prepare_lines(script)?;
    let mut pos = 0;
    // 顶层缩进为 0，故用 -1 作为父级阈值，保证顶层语句也能进入块。
    parse_block(&lines, &mut pos, -1)
}

fn prepare_lines(script: &str) -> Result<Vec<Line>, String> {
    let mut out = Vec::new();
    for raw in script.lines() {
        let indent = raw.len() - raw.trim_start().len();
        let content = raw.trim();
        if content.is_empty() {
            continue;
        }
        let toks = tokenize(content)?;
        if toks.is_empty() {
            continue;
        }
        out.push(Line { indent, toks });
    }
    Ok(out)
}

/// 递归解析一条语句。返回该语句并推进 `pos`。
fn parse_stmt(lines: &[Line], pos: &mut usize) -> Result<Stmt, String> {
    let header = lines[*pos].toks.clone();
    let header_indent = lines[*pos].indent;
    let first = header
        .get(0)
        .and_then(|t| match t {
            Tok::Ident(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default();
    match first.as_str() {
        "if" => {
            let mut p = Parser::new(header);
            p.next();
            let cond = p.expr()?;
            *pos += 1;
            let then = parse_block(lines, pos, header_indent as i64)?;
            // else：若下一行缩进等于 header 且关键字为 else
            let els = if lines
                .get(*pos)
                .map(|l| l.indent == header_indent && is_tok_kw(&l.toks, "else"))
                .unwrap_or(false)
            {
                *pos += 1; // 吃 else
                parse_block(lines, pos, header_indent as i64)?
            } else {
                Vec::new()
            };
            Ok(Stmt::If { cond, then, els })
        }
        "for" => {
            let mut p = Parser::new(header);
            p.next();
            let var = match p.next() {
                Some(Tok::Ident(v)) if !is_kw(&v) => v,
                _ => return Err("for 后需循环变量".into()),
            };
            if !p.eat_ident("in") {
                return Err("for 需写作 for x in <列表/数值>".into());
            }
            let items = p.expr()?;
            *pos += 1;
            let body = parse_block(lines, pos, header_indent as i64)?;
            Ok(Stmt::For { var, n: items, body })
        }
        "while" => {
            let mut p = Parser::new(header);
            p.next();
            let cond = p.expr()?;
            *pos += 1;
            let body = parse_block(lines, pos, header_indent as i64)?;
            Ok(Stmt::While { cond, body })
        }
        "break" => {
            *pos += 1;
            Ok(Stmt::Break)
        }
        "continue" => {
            *pos += 1;
            Ok(Stmt::Continue)
        }
        _ => {
            let stmt = parse_simple(&header)?;
            *pos += 1;
            Ok(stmt)
        }
    }
}

fn is_tok_kw(toks: &[Tok], kw: &str) -> bool {
    matches!(toks.get(0), Some(Tok::Ident(k)) if k == kw)
}

/// 解析 `ident = expr` 赋值，否则视为表达式（函数调用）语句。
fn parse_simple(header: &[Tok]) -> Result<Stmt, String> {
    // 尝试识别 `ident = ...`
    if let Some(Tok::Ident(name)) = header.get(0) {
        if matches!(header.get(1), Some(Tok::Op(o)) if o == "=") {
            let mut p = Parser::new(header.get(2..).unwrap_or(&[]).to_vec());
            let e = p.expr()?;
            return Ok(Stmt::Assign(name.clone(), e));
        }
    }
    let mut p = Parser::new(header.to_vec());
    let e = p.expr()?;
    Ok(Stmt::Expr(e))
}

/// 解析属于 `parent_indent` 之下（缩进更深）的连续语句块。
/// 块内允许的缩进只要大于父级阈值即可；遇到回到父级缩进的 `end` 或语句时结束。
fn parse_block(lines: &[Line], pos: &mut usize, parent_indent: i64) -> Result<Vec<Stmt>, String> {
    let mut stmts = Vec::new();
    while *pos < lines.len() {
        let l = &lines[*pos];
        // 单独的 `end`：块的显式终止符，作为空操作跳过。
        if l.toks.len() == 1 && is_tok_kw(&l.toks, "end") {
            *pos += 1;
            continue;
        }
        if (l.indent as i64) <= parent_indent {
            break;
        }
        let s = parse_stmt(lines, pos)?;
        stmts.push(s);
    }
    Ok(stmts)
}

// ---------------------------------------------------------------------------
// 表达式递归下降
// ---------------------------------------------------------------------------

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn new(toks: Vec<Tok>) -> Self {
        Parser { toks, pos: 0 }
    }
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }
    fn is_ident(&self, name: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(k)) if k == name)
    }
    fn eat_ident(&mut self, name: &str) -> bool {
        if self.is_ident(name) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn is_op(&self, name: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if o == name)
    }
    fn eat_op(&mut self, name: &str) -> bool {
        if self.is_op(name) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.or_expr()
    }
    fn or_expr(&mut self) -> Result<Expr, String> {
        let mut l = self.and_expr()?;
        while self.eat_op("||") {
            let r = self.and_expr()?;
            l = Expr::Bin("||".into(), Box::new(l), Box::new(r));
        }
        Ok(l)
    }
    fn and_expr(&mut self) -> Result<Expr, String> {
        let mut l = self.not_expr()?;
        while self.eat_op("&&") {
            let r = self.not_expr()?;
            l = Expr::Bin("&&".into(), Box::new(l), Box::new(r));
        }
        Ok(l)
    }
    fn not_expr(&mut self) -> Result<Expr, String> {
        if self.eat_op("!") || self.eat_ident("not") {
            Ok(Expr::Not(Box::new(self.not_expr()?)))
        } else {
            self.cmp_expr()
        }
    }
    fn cmp_expr(&mut self) -> Result<Expr, String> {
        let mut l = self.add_expr()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Op(o)) if matches!(o.as_str(), "==" | "!=" | "<" | "<=" | ">" | ">=") => {
                    let o = o.clone();
                    self.pos += 1;
                    o
                }
                _ => break,
            };
            let r = self.add_expr()?;
            l = Expr::Bin(op, Box::new(l), Box::new(r));
        }
        Ok(l)
    }
    fn add_expr(&mut self) -> Result<Expr, String> {
        let mut l = self.mul_expr()?;
        loop {
            if self.eat_op("+") {
                let r = self.mul_expr()?;
                l = Expr::Bin("+".into(), Box::new(l), Box::new(r));
            } else if self.eat_op("-") {
                let r = self.mul_expr()?;
                l = Expr::Bin("-".into(), Box::new(l), Box::new(r));
            } else {
                break;
            }
        }
        Ok(l)
    }
    fn mul_expr(&mut self) -> Result<Expr, String> {
        let mut l = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Op(o)) if matches!(o.as_str(), "*" | "/" | "%") => {
                    let o = o.clone();
                    self.pos += 1;
                    o
                }
                _ => break,
            };
            let r = self.unary()?;
            l = Expr::Bin(op, Box::new(l), Box::new(r));
        }
        Ok(l)
    }
    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat_op("-") {
            Ok(Expr::Neg(Box::new(self.unary()?)))
        } else {
            self.atom()
        }
    }
    fn atom(&mut self) -> Result<Expr, String> {
        let t = self.next().ok_or("表达式提前结束")?;
        match t {
            Tok::Num(n) => Ok(Expr::Lit(V::N(n))),
            Tok::Str(s) => Ok(Expr::Lit(V::S(s))),
            Tok::Ident(name) => {
                if self.eat_op("(") {
                    let mut args = Vec::new();
                    if !self.eat_op(")") {
                        loop {
                            args.push(self.expr()?);
                            if self.eat_op(",") {
                                continue;
                            }
                            if !self.eat_op(")") {
                                return Err("函数参数缺右括号".into());
                            }
                            break;
                        }
                    }
                    Ok(Expr::Call(name, args))
                } else {
                    match name.as_str() {
                        "true" => Ok(Expr::Lit(V::B(true))),
                        "false" => Ok(Expr::Lit(V::B(false))),
                        k if is_kw(k) => Err(format!("意外的关键字: {}", k)),
                        _ => Ok(Expr::Var(name)),
                    }
                }
            }
            Tok::Op(o) if o == "(" => {
                let e = self.expr()?;
                if !self.eat_op(")") {
                    return Err("缺右括号".into());
                }
                Ok(e)
            }
            _ => Err("无法解析的表达式".into()),
        }
    }
}

fn is_kw(s: &str) -> bool {
    matches!(
        s,
        "if" | "else" | "end" | "for" | "in" | "range" | "while" | "break" | "continue" | "and" | "or" | "not" | "true" | "false"
    )
}

// ---------------------------------------------------------------------------
// 解释执行
// ---------------------------------------------------------------------------

/// 循环/总指令执行上限，防止死循环拖垮面板。
const MAX_ITER: u64 = 1_000_000;

pub struct RunOutcome {
    pub value: String,
    pub logs: Vec<String>,
}

pub struct Interp<'a> {
    vars: HashMap<String, V>,
    retval: Option<String>,
    logs: Vec<String>,
    builtin: &'a dyn Builtin,
    steps: u64,
    kv_writes: Vec<(String, String)>,
    kv_prefix: String,
}

impl<'a> Interp<'a> {
    pub fn new(builtin: &'a dyn Builtin) -> Self {
        Interp::with_prefix("".to_string(), builtin)
    }

    /// 指定 KV 命名空间前缀，用于隔离不同插件的键。
    pub fn with_prefix(prefix: String, builtin: &'a dyn Builtin) -> Self {
        Interp {
            vars: HashMap::new(),
            retval: None,
            logs: Vec::new(),
            builtin,
            steps: 0,
            kv_writes: Vec::new(),
            kv_prefix: prefix,
        }
    }

    pub fn run(&mut self, script: &str) -> Result<RunOutcome, String> {
        let stmts = parse(script)?;
        self.exec(&stmts)?;
        Ok(RunOutcome {
            value: self.retval.clone().unwrap_or_default(),
            logs: self.logs.clone(),
        })
    }

    /// 本次执行产生的 kv_set 写入列表，供宿主持久化。
    pub fn take_kv_writes(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.kv_writes)
    }

    fn kv_key(&self, raw: &str) -> String {
        if self.kv_prefix.is_empty() {
            raw.to_string()
        } else {
            format!("{}:{}", self.kv_prefix, raw)
        }
    }

    fn tick(&mut self) -> Result<(), String> {
        self.steps += 1;
        if self.steps > MAX_ITER {
            return Err("执行超上限（可能死循环），已中止".into());
        }
        Ok(())
    }

    fn exec(&mut self, stmts: &[Stmt]) -> Result<(), String> {
        for s in stmts {
            self.tick()?;
            match s {
                Stmt::Assign(name, e) => {
                    let v = self.eval(e)?;
                    self.vars.insert(name.clone(), v);
                }
                Stmt::Expr(e) => {
                    self.eval(e)?;
                }
                Stmt::If { cond, then, els } => {
                    let c = self.eval(cond)?;
                    if c.truthy() {
                        self.exec(then)?;
                    } else {
                        self.exec(els)?;
                    }
                }
                Stmt::For { var, n, body } => {
                    let iv = self.eval(n)?;
                    if let Some(items) = iv.list() {
                        let items: Vec<String> = items.to_vec();
                        for it in items {
                            self.tick()?;
                            self.vars.insert(var.clone(), V::S(it));
                            match self.exec(body) {
                                Ok(()) => {}
                                Err(e) if e == "!break" => break,
                                Err(e) if e == "!continue" => continue,
                                Err(e) => return Err(e),
                            }
                        }
                    } else {
                        let cnt = iv.as_num() as i64;
                        if cnt > 0 {
                            for i in 0..cnt {
                                self.tick()?;
                                self.vars.insert(var.clone(), V::N(i as f64));
                                match self.exec(body) {
                                    Ok(()) => {}
                                    Err(e) if e == "!break" => break,
                                    Err(e) if e == "!continue" => continue,
                                    Err(e) => return Err(e),
                                }
                            }
                        }
                    }
                }
                Stmt::While { cond, body } => {
                    while self.eval(cond)?.truthy() {
                        self.tick()?;
                        match self.exec(body) {
                            Ok(()) => {}
                            Err(e) if e == "!break" => break,
                            Err(e) if e == "!continue" => continue,
                            Err(e) => return Err(e),
                        }
                    }
                }
                Stmt::Break => return Err("!break".into()),
                Stmt::Continue => return Err("!continue".into()),
            }
        }
        Ok(())
    }

    fn eval(&mut self, e: &Expr) -> Result<V, String> {
        self.tick()?;
        match e {
            Expr::Lit(v) => Ok(v.clone()),
            Expr::Var(name) => Ok(self.vars.get(name).cloned().unwrap_or(V::Nil)),
            Expr::Not(x) => Ok(V::B(!self.eval(x)?.truthy())),
            Expr::Neg(x) => Ok(V::N(-self.eval(x)?.as_num())),
            Expr::Bin(op, a, b) => self.eval_bin(op, a, b),
            Expr::Call(name, args) => self.call(name, args),
        }
    }

    fn eval_bin(&mut self, op: &str, a: &Expr, b: &Expr) -> Result<V, String> {
        let av = self.eval(a)?;
        let bv = self.eval(b)?;
        match op {
            "+" => {
                // 任一为字符串则拼接，否则数值加
                if matches!(av, V::S(_)) || matches!(bv, V::S(_)) {
                    Ok(V::S(av.text() + &bv.text()))
                } else {
                    Ok(V::N(av.as_num() + bv.as_num()))
                }
            }
            "-" => Ok(V::N(av.as_num() - bv.as_num())),
            "*" => Ok(V::N(av.as_num() * bv.as_num())),
            "/" => {
                let d = bv.as_num();
                if d == 0.0 {
                    Ok(V::S("err:div0".into()))
                } else {
                    Ok(V::N(av.as_num() / d))
                }
            }
            "%" => Ok(V::N(av.as_num() % bv.as_num())),
            "==" => Ok(V::B(av == bv)),
            "!=" => Ok(V::B(av != bv)),
            "<" => Ok(V::B(cmp_lt(&av, &bv))),
            "<=" => Ok(V::B(cmp_lt(&av, &bv) || av == bv)),
            ">" => Ok(V::B(cmp_lt(&bv, &av))),
            ">=" => Ok(V::B(cmp_lt(&bv, &av) || av == bv)),
            "&&" => Ok(V::B(av.truthy() && bv.truthy())),
            "||" => Ok(V::B(av.truthy() || bv.truthy())),
            _ => Err(format!("未知运算符: {}", op)),
        }
    }

    fn call(&mut self, name: &str, args: &[Expr]) -> Result<V, String> {
        let evaled: Vec<V> = args.iter().map(|a| self.eval(a)).collect::<Result<_, _>>()?;
        let one = evaled.get(0).cloned().unwrap_or(V::Nil);
        let two = evaled.get(1).cloned().unwrap_or(V::Nil);
        let three = evaled.get(2).cloned().unwrap_or(V::Nil);
        let s = |v: &V| v.text();
        match name {
            "cmd" => Ok(V::S(self.builtin.cmd(&s(&one)))),
            "fetch" => {
                let t = two.as_num() as u64;
                let t = if t == 0 { 8 } else { t };
                Ok(V::S(self.builtin.fetch(&s(&one), t)))
            }
            "post" => Ok(V::S(self.builtin.post(&s(&one), &s(&two)))),
            "http_status" => Ok(V::S(self.builtin.http_status(&s(&one)))),
            "ret" => {
                self.retval = Some(s(&one));
                Ok(V::Nil)
            }
            "log" => {
                self.logs.push(s(&one));
                Ok(V::Nil)
            }
            "env" => Ok(V::S(std::env::var(&s(&one)).unwrap_or_default())),
            "var" => Ok(self.vars.get(&s(&one)).cloned().unwrap_or(V::Nil)),
            "arg" => Ok(V::S(self.builtin.arg(&s(&one)).unwrap_or_default())),
            "has_arg" => Ok(V::B(self.builtin.has_arg(&s(&one)))),
            "kv_get" => {
                let k = self.kv_key(&s(&one));
                Ok(V::S(self.builtin.kv_get(&k).unwrap_or_default()))
            }
            "kv_set" => {
                let k = self.kv_key(&s(&one));
                let v = s(&two);
                let r = self.builtin.kv_set(&k, &v);
                self.kv_writes.push((k.clone(), v));
                Ok(V::B(r))
            }
            "len" => {
                if let Some(v) = one.list() {
                    Ok(V::N(v.len() as f64))
                } else {
                    Ok(V::N(s(&one).chars().count() as f64))
                }
            }
            "substr" => {
                let start = two.as_num() as usize;
                let end = three.as_num();
                let chars: Vec<String> = s(&one).chars().map(|c| c.to_string()).collect();
                let n = chars.len();
                let start = start.min(n);
                let end = if end < 0.0 {
                    n
                } else {
                    (end as usize).min(n)
                };
                Ok(V::S(chars[start..end.max(start)].concat()))
            }
            "atoi" => Ok(V::N(one.as_num())),
            "itoa" => Ok(V::S(one.text())),
            "min" => {
                let a = one.as_num();
                let b = two.as_num();
                Ok(V::N(a.min(b)))
            }
            "max" => {
                let a = one.as_num();
                let b = two.as_num();
                Ok(V::N(a.max(b)))
            }
            "round" => Ok(V::N(one.as_num().round())),
            "ceil" => Ok(V::N(one.as_num().ceil())),
            "floor" => Ok(V::N(one.as_num().floor())),
            "upper" => Ok(V::S(s(&one).to_uppercase())),
            "lower" => Ok(V::S(s(&one).to_lowercase())),
            "trim" => Ok(V::S(s(&one).trim().to_string())),
            "split" => {
                let parts: Vec<String> = s(&one).split(&s(&two)).map(|p| p.to_string()).collect();
                Ok(V::S(parts.join("|")))
            }
            "json" => Ok(V::S(json_quote(&s(&one)))),

            // ---- 列表 / 迭代 ----
            "range" => Ok(V::N(one.as_num().floor().max(0.0))),
            "lines" => {
                let v: Vec<String> = s(&one)
                    .split('\n')
                    .map(|l| l.trim_end_matches('\r').to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
                Ok(V::L(v))
            }
            "split_list" => Ok(V::L(s(&one).split(&s(&two)).map(|p| p.to_string()).collect())),
            "at" => {
                let i = two.as_num() as i64;
                let v = one.list().map(|lst| lst.get(i as usize).cloned()).flatten().unwrap_or_default();
                Ok(V::S(v))
            }
            "push" => {
                let mut v = one.list().unwrap_or(&[]).to_vec();
                v.push(s(&two));
                Ok(V::L(v))
            }
            "pop" => {
                let mut v = one.list().unwrap_or(&[]).to_vec();
                let last = v.pop().unwrap_or_default();
                // 用原子 KV 之外的临时手段：把剩余列表放回 var? 不行——改用追加内联。
                let _ = &mut v;
                Ok(V::S(last))
            }
            "join" => Ok(V::S(one.list().unwrap_or(&[]).join(&s(&two)))),

            // ---- 文件操作 ----
            "read_file" => Ok(V::S(self.builtin.read_file(&s(&one)))),
            "write_file" => Ok(V::B(self.builtin.write_file(&s(&one), &s(&two)))),
            "append_file" => Ok(V::B(self.builtin.append_file(&s(&one), &s(&two)))),
            "ls" => Ok(V::S(self.builtin.ls(&s(&one)))),
            "file_info" => Ok(V::S(self.builtin.file_info(&s(&one)))),
            "rm" => {
                let p = s(&one);
                Ok(V::B(std::fs::remove_file(&p).is_ok() || std::fs::remove_dir_all(&p).is_ok()))
            }

            // ---- 系统 / 网络 ----
            "lookup_ip" => Ok(V::S(self.builtin.lookup_ip(&s(&one)))),
            "shasum" => Ok(V::S(self.builtin.sha1(&s(&one)))),
            "urlenc" => Ok(V::S(self.builtin.urlenc(&s(&one)))),
            "kill" => { let p = one.as_num() as u32; Ok(V::B(self.builtin.kill_pid(p))) }
            "sleep" => {
                let ms = two.as_num() as u64;
                if ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(ms.min(60_000)));
                }
                Ok(V::Nil)
            }
            "now" => Ok(V::N(now_epoch() as f64)),
            "date" => Ok(V::S(fmt_time(now_epoch(), "%Y-%m-%d %H:%M:%S"))),
            "date_fmt" => {
                let f = if s(&one).is_empty() { "%Y-%m-%d %H:%M:%S" } else { &s(&one) };
                Ok(V::S(fmt_time(now_epoch(), f)))
            }
            "strftime" => {
                let t = two.as_num() as i64;
                let f = if s(&one).is_empty() { "%Y-%m-%d %H:%M:%S" } else { &s(&one) };
                Ok(V::S(fmt_time(t, f)))
            }
            "rand" => {
                let lo = one.as_num();
                let hi = if evaled.len() > 1 { two.as_num() } else { lo };
                Ok(V::N(rand_range(lo, hi)))
            }

            // ---- 字符串增强 ----
            "contains" => Ok(V::B(s(&one).contains(&s(&two)))),
            "startswith" => Ok(V::B(s(&one).starts_with(&s(&two)))),
            "endswith" => Ok(V::B(s(&one).ends_with(&s(&two)))),
            "index" => {
                let p = s(&one).find(&s(&two)).map(|i| i as f64).unwrap_or(-1.0);
                Ok(V::N(p))
            }
            "replace" => Ok(V::S(s(&one).replace(&s(&two), &s(&three)))),
            "rev" => Ok(V::S(s(&one).chars().rev().collect())),
            "count" => {
                let n = s(&one).matches(&s(&two)).count() as f64;
                Ok(V::N(n))
            }
            "pad" => {
                let me = s(&one);
                let w = two.as_num() as usize;
                if me.chars().count() >= w {
                    Ok(V::S(me))
                } else {
                    let pad = " ".repeat(w - me.chars().count());
                    Ok(V::S(pad + &me))
                }
            }

            // ---- 新增：JSON 读取 / 格式化 / 列表 / 编解码 ----
            "json_get" => Ok(V::S(json_get_value(&s(&one), &s(&two)).unwrap_or_default())),
            "keys" => {
                let ks = object_keys(&s(&one));
                Ok(V::L(ks))
            }
            "compact" => Ok(V::S(json_compact(&s(&one)))),
            "fmt_bytes" => Ok(V::S(fmt_bytes(one.as_num()))),
            "fmt_dur" => Ok(V::S(fmt_dur(one.as_num()))),
            "sortlist" => {
                let mut v = one.list().unwrap_or(&[]).to_vec();
                v.sort();
                Ok(V::L(v))
            }
            "uniq" => {
                let mut seen = std::collections::HashSet::new();
                let mut v: Vec<String> = Vec::new();
                for it in one.list().unwrap_or(&[]) {
                    if seen.insert(it.clone()) {
                        v.push(it.clone());
                    }
                }
                Ok(V::L(v))
            }
            "words" => {
                let w: Vec<String> = s(&one).split_whitespace().map(|p| p.to_string()).collect();
                Ok(V::L(w))
            }
            "b64" => Ok(V::S(base64_encode(s(&one).as_bytes()))),
            "b64d" => Ok(V::S(String::from_utf8_lossy(&base64_decode(&s(&one))).into_owned())),
            "glob" => Ok(V::B(glob_match(&s(&one), &s(&two)))),
            "repeat" => Ok(V::S(s(&one).repeat(two.as_num().max(0.0) as usize))),

            // ---- 更多增强：哈希 / URL / 路径 / 统计 / 工具 ----
            "sha256" => Ok(V::S(hex_of(&sha256_digest(s(&one).as_bytes())))),
            "urldec" => Ok(V::S(urldecode(&s(&one)))),
            "basename" => Ok(V::S(
                s(&one).rsplit('/').next().map(|p| p.trim_end_matches('/').to_string()).unwrap_or_default(),
            )),
            "dirname" => {
                let p = s(&one);
                let e = p.rsplit('/').next().map(|p| p.trim_end_matches('/').to_string()).unwrap_or_default();
                let d = &p[..p.len() - e.len()];
                Ok(V::S(d.trim_end_matches('/').to_string()))
            }
            "extname" => {
                let b = s(&one).rsplit('/').next().unwrap_or_default().to_string();
                let idx = b.rfind('.');
                Ok(V::S(match idx {
                    Some(i) if i > 0 => b[i + 1..].to_string(),
                    _ => String::new(),
                }))
            }
            "sum" => {
                let mut t = 0.0;
                for it in one.list().unwrap_or(&[]) {
                    t += parse_num(it);
                }
                Ok(V::N(t))
            }
            "avg" => {
                let a = one.list().unwrap_or(&[]);
                if a.is_empty() {
                    Ok(V::N(0.0))
                } else {
                    let mut t = 0.0;
                    for it in a {
                        t += parse_num(it);
                    }
                    Ok(V::N(t / a.len() as f64))
                }
            }
            "pct" => {
                let p = one.as_num();
                let t = two.as_num();
                if t == 0.0 {
                    Ok(V::N(0.0))
                } else {
                    Ok(V::N(round1((p / t) * 100.0)))
                }
            }
            "uuid" => Ok(V::S(uuid4())),
            "hostname" => {
                let hn = std::fs::read_to_string("/proc/sys/kernel/hostname")
                    .map(|s| s.trim().to_string())
                    .or_else(|_| std::env::var("HOSTNAME"))
                    .unwrap_or_default();
                Ok(V::S(hn))
            }
            "hexenc" => Ok(V::S(hex_of(s(&one).as_bytes()))),
            "hexdec" => Ok(V::S(String::from_utf8_lossy(&hex_decode(&s(&one))).into_owned())),
            "esc" => Ok(V::S(sh_quote(&s(&one)))),
            "unsh" => Ok(V::S(json_quote(&s(&one)))),

            other => Err(format!("未知函数: {}", other)),
        }
    }
}

/// 数值优先比较，否则按字符串比较。
fn cmp_lt(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::N(x), V::N(y)) => x < y,
        (V::S(x), V::S(y)) => x < y,
        _ => a.as_num() < b.as_num(),
    }
}

/// 当前 unix 秒（UTC）。
fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 时间戳按格式字符串格式化（UTC 的近似本地化，偏移见 config::tz()）。
fn fmt_time(epoch: i64, fmt: &str) -> String {
    let offset = *crate::config::tz();
    let local = epoch + offset;
    let days = local.div_euclid(86400);
    let secs = local.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    let wd = ((days + 4) % 7).rem_euclid(7); // 1970-01-01 是周四(4)
    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('Y') => out.push_str(&format!("{:04}", y)),
            Some('y') => out.push_str(&format!("{:02}", y % 100)),
            Some('m') => out.push_str(&format!("{:02}", m)),
            Some('d') => out.push_str(&format!("{:02}", d)),
            Some('H') => out.push_str(&format!("{:02}", hh)),
            Some('M') => out.push_str(&format!("{:02}", mm)),
            Some('S') => out.push_str(&format!("{:02}", ss)),
            Some('w') => out.push_str(&format!("{}", wd)),
            Some('j') => {
                let doe = days - civil_yoe_days(y);
                out.push_str(&format!("{:03}", doe));
            }
            Some('%') => out.push('%'),
            Some(o) => {
                out.push('%');
                out.push(o);
            }
            None => out.push('%'),
        }
    }
    out
}

/// 计算给定年份 1 月 1 日的儒略日（用于 %j，仅为近似，配合应急用可接受）。
fn civil_yoe_days(y: i64) -> i64 {
    let mut days = 0;
    let mut yy = 1970;
    while yy < y {
        let leap = (yy % 4 == 0 && yy % 100 != 0) || yy % 400 == 0;
        days += if leap { 366 } else { 365 };
        yy += 1;
    }
    days
}

/// 民用历 → 年/月/日（1970 纪元）。
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
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

/// 生成 [lo, hi] 闭区间内的随机整数。
fn rand_range(lo: f64, hi: f64) -> f64 {
    if hi <= lo {
        return lo;
    }
    // 用一个简单的线性同余伪随机器，避免依赖。
    use std::time::{SystemTime, UNIX_EPOCH};
    static mut SEED: u64 = 0;
    unsafe {
        if SEED == 0 {
            SEED = SystemTime::now().duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(88172645463325252);
        }
        SEED = SEED.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let span = (hi.floor() as i64 - lo.ceil() as i64).max(0) + 1;
        let r = (SEED >> 33) % (span as u64);
        (lo.ceil() as i64 + r as i64) as f64
    }
}

/// 生成一个 JSON 字符串字面量（用于 `ret(json("..."))` 返回结构化）。
fn json_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// 新增工具函数（供插件 DSL 调用）
// ---------------------------------------------------------------------------

/// 取 JSON 中 `path`（点路径，如 `a.b` / `list[0]`）对应值的文本表示。
/// 字符串自动去引号、对象/数组合并成紧凑文本。
fn json_get_value(json: &str, path: &str) -> Option<String> {
    let mut cur = json.trim().to_string();
    for seg in path.split('.') {
        let (key, idx) = match seg.split_once('[') {
            Some((k, rest)) => {
                let i = rest.trim_end_matches(']').trim().parse::<usize>().ok();
                (k, i)
            }
            None => (seg, None),
        };
        let needle = format!("\"{}\"", key);
        let mut pos = 0;
        let value = loop {
            let rel = cur[pos..].find(&needle)?;
            let s0 = pos + rel;
            let after = cur[s0 + needle.len()..].trim_start();
            if after.starts_with(':') {
                break after[1..].trim_start();
            }
            pos = s0 + needle.len();
        };
        let raw = take_json_value(value)?;
        // 数组下标：只在 raw 为数组时按逗号分割取第 idx 个元素。
        if let Some(i) = idx {
            let raw = if raw.starts_with('[') { &raw[1..raw.len() - 1] } else { raw };
            let elems = split_top_level(raw);
            let v = elems.get(i).copied().unwrap_or("");
            cur = optional_unquote(v.trim()).to_string();
        } else if raw.trim_start().starts_with('{') || raw.trim_start().starts_with('[') {
            // 仍有嵌套 → 继续向下传
            cur = raw.to_string();
        } else {
            cur = optional_unquote(raw.trim()).to_string();
        }
    }
    if cur.trim_start().starts_with('{') || cur.trim_start().starts_with('[') {
        Some(json_compact(&cur))
    } else {
        Some(cur)
    }
}

/// 取 JSON 中紧跟冒号后的一个完整值（按引号/转义/嵌套平衡切分）。
fn take_json_value(s: &str) -> Option<&str> {
    let t = s.trim_start();
    let first = t.chars().next()?;
    match first {
        '{' | '[' => {
            let mut depth = 0i32;
            let mut in_str = false;
            let mut esc = false;
            for (i, c) in t.char_indices() {
                if esc {
                    esc = false;
                    continue;
                }
                if c == '\\' {
                    esc = true;
                    continue;
                }
                if c == '"' {
                    in_str = !in_str;
                    continue;
                }
                if in_str {
                    continue;
                }
                match c {
                    '{' | '[' => depth += 1,
                    '}' | ']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(&t[..=i]);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        '"' => {
            let mut esc = false;
            for (i, c) in t.char_indices().skip(1) {
                if esc {
                    esc = false;
                    continue;
                }
                if c == '\\' {
                    esc = true;
                    continue;
                }
                if c == '"' {
                    return Some(&t[..=i]);
                }
            }
            None
        }
        _ => {
            let mut end = t.len();
            let mut esc = false;
            let mut in_quote = false;
            for (i, c) in t.char_indices() {
                if esc {
                    esc = false;
                    continue;
                }
                if c == '\\' {
                    esc = true;
                    continue;
                }
                if c == '"' {
                    in_quote = !in_quote;
                    continue;
                }
                if !in_quote && (c == ',' || c == '}' || c == ']' || c == '\n' || c == '\r') {
                    end = i;
                    break;
                }
            }
            Some(&t[..end])
        }
    }
}

/// 顶层逗号分割（忽略引号内与嵌套括号），用于取数组元素。
fn split_top_level(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, c) in s.char_indices() {
        if esc {
            esc = false;
            continue;
        }
        if c == '\\' {
            esc = true;
            continue;
        }
        if c == '"' {
            in_str = !in_str;
            continue;
        }
        if in_str {
            continue;
        }
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// 字符串值去掉首尾引号并解转义；非字符串原样返回。
fn optional_unquote(v: &str) -> String {
    let t = v.trim();
    let bytes = t.as_bytes();
    if !t.starts_with('"') || bytes.len() < 2 || *bytes.last().unwrap() != b'"' {
        return t.to_string();
    }
    let inner = &t[1..t.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// 压缩 JSON：去掉引号外的空白。
fn json_compact(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_str = false;
    let mut esc = false;
    for c in s.chars() {
        if esc {
            out.push(c);
            esc = false;
            continue;
        }
        if c == '\\' {
            out.push(c);
            esc = true;
            continue;
        }
        if c == '"' {
            in_str = !in_str;
            out.push(c);
            continue;
        }
        if in_str {
            out.push(c);
            continue;
        }
        if c.is_whitespace() {
            continue;
        }
        out.push(c);
    }
    out
}

/// 取对象顶层键名列表。
fn object_keys(s: &str) -> Vec<String> {
    let t = s.trim();
    let mut keys = Vec::new();
    if !t.starts_with('{') {
        return keys;
    }
    let b = t.as_bytes();
    let mut i = 0;
    let mut in_str = false;
    let mut esc = false;
    let mut depth = 0i32;
    while i < b.len() {
        let c = b[i];
        if esc {
            esc = false;
            i += 1;
            continue;
        }
        if c == b'\\' {
            esc = true;
            i += 1;
            continue;
        }
        if c == b'"' {
            if depth == 1 && !in_str {
                let mut j = i + 1;
                let mut k = String::new();
                esc = false;
                while j < b.len() {
                    let cj = b[j];
                    if esc {
                        k.push(cj as char);
                        esc = false;
                        j += 1;
                        continue;
                    }
                    if cj == b'\\' {
                        k.push(cj as char);
                        esc = true;
                        j += 1;
                        continue;
                    }
                    if cj == b'"' {
                        break;
                    }
                    k.push(cj as char);
                    j += 1;
                }
                let mut p = j + 1;
                while p < b.len() && (b[p] == b' ' || b[p] == b'\t' || b[p] == b'\n' || b[p] == b'\r') {
                    p += 1;
                }
                if p < b.len() && b[p] == b':' {
                    keys.push(k);
                }
                i = j + 1;
                continue;
            }
            in_str = !in_str;
            i += 1;
            continue;
        }
        if !in_str {
            match c {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
        }
        i += 1;
    }
    keys
}

/// 人类可读的字节大小，如 `1.5G` / `820M`。
fn fmt_bytes(n: f64) -> String {
    if n < 0.0 {
        return "0B".to_string();
    }
    if n < 1024.0 {
        return format!("{}B", n as i64);
    }
    let units = ["K", "M", "G", "T", "P"];
    let mut v = n / 1024.0;
    let mut u = 0;
    while v >= 1024.0 && u < units.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{:.1}{}", v, units[u])
}

/// 人类可读的时长，如 `3d 4h 5m` / `1h 2m` / `45s`。
fn fmt_dur(sec: f64) -> String {
    let mut s = sec.floor().max(0.0) as i64;
    let d = s / 86400;
    s %= 86400;
    let h = s / 3600;
    s %= 3600;
    let m = s / 60;
    s %= 60;
    if d > 0 {
        format!("{}d {}h {}m", d, h, m)
    } else if h > 0 {
        format!("{}h {}m", h, m)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

/// Base64 编码（标准字母表 + `=` 填充）。
fn base64_encode(d: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((d.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= d.len() {
        let n = ((d[i] as u32) << 16) | ((d[i + 1] as u32) << 8) | (d[i + 2] as u32);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(T[(n >> 6) as usize & 63] as char);
        out.push(T[n as usize & 63] as char);
        i += 3;
    }
    let rem = d.len() - i;
    if rem == 1 {
        let n = (d[i] as u32) << 16;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((d[i] as u32) << 16) | ((d[i + 1] as u32) << 8);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(T[(n >> 6) as usize & 63] as char);
        out.push('=');
    }
    out
}

/// Base64 解码（容错空白，忽略非法字符）。
fn base64_decode(s: &str) -> Vec<u8> {
    let val = |c: u8| -> i32 {
        match c {
            b'A'..=b'Z' => (c - b'A') as i32,
            b'a'..=b'z' => (c - b'a' + 26) as i32,
            b'0'..=b'9' => (c - b'0' + 52) as i32,
            b'+' => 62,
            b'/' => 63,
            _ => -1,
        }
    };
    let b: Vec<u8> = s
        .bytes()
        .filter(|c| !matches!(c, b'=' | b'\n' | b'\r' | b' ' | b'\t'))
        .collect();
    let mut out = Vec::with_capacity(b.len() / 4 * 3);
    let clear = |i: usize| val(b[i]) as u32;
    let mut i = 0;
    while i + 4 <= b.len() {
        let n = (clear(i) << 18) | (clear(i + 1) << 12) | (clear(i + 2) << 6) | clear(i + 3);
        out.push((n >> 16) as u8);
        out.push((n >> 8) as u8);
        out.push(n as u8);
        i += 4;
    }
    let rem = b.len() - i;
    if rem >= 3 {
        let n = (clear(i) << 18) | (clear(i + 1) << 12) | (clear(i + 2) << 6);
        out.push((n >> 16) as u8);
        out.push((n >> 8) as u8);
    } else if rem == 2 {
        let n = (clear(i) << 18) | (clear(i + 1) << 12);
        out.push((n >> 16) as u8);
    }
    out
}

/// 解析字符串为数字（兼容 `1.5G`、`512K` 等带单位值）。
fn parse_num(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    let (val, mult) = match t.as_bytes().last() {
        Some(&b'k') | Some(&b'K') => (&t[..t.len() - 1], 1024.0),
        Some(&b'm') | Some(&b'M') => (&t[..t.len() - 1], 1024.0 * 1024.0),
        Some(&b'g') | Some(&b'G') => (&t[..t.len() - 1], 1024.0 * 1024.0 * 1024.0),
        Some(&b'%') => (&t[..t.len() - 1], 1.0),
        _ => (t, 1.0),
    };
    val.trim().parse::<f64>().unwrap_or(0.0) * mult
}

/// 保留一位小数。
fn round1(n: f64) -> f64 {
    (n * 10.0).round() / 10.0
}

/// 生成随机 v4 UUID。
fn uuid4() -> String {
    let mut rng = tiny_rand();
    let mut b = [0u8; 16];
    for x in b.iter_mut() {
        *x = rng();
    }
    b[6] = (b[6] & 0x0f) | 0x40; // 版本 4
    b[8] = (b[8] & 0x3f) | 0x80; // 变体
    hex_of(&b[0..4]) + "-" + &hex_of(&b[4..6]) + "-" + &hex_of(&b[6..8]) + "-"
        + &hex_of(&b[8..10]) + "-" + &hex_of(&b[10..16])
}

/// 极简确定性伪随机（线性同余），每次脚本新建。
fn tiny_rand() -> impl FnMut() -> u8 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEED: AtomicU32 = AtomicU32::new(0x9e3779b9);
    let seed = SEED.fetch_add(0x9e3779b9, Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u32)
        .unwrap_or(0);
    let mut state = seed.rotate_left(13).wrapping_add(now);
    state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    move || {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        (state >> 16) as u8
    }
}

/// 字节 → 小写十六进制。
fn hex_of(d: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(d.len() * 2);
    for &x in d {
        out.push(H[(x >> 4) as usize] as char);
        out.push(H[(x & 0x0f) as usize] as char);
    }
    out
}

/// 十六进制字符串 → 字节（忽略非法字符）。
fn hex_decode(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() / 2);
    let mut hi: Option<u8> = None;
    for c in s.bytes() {
        let v = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => continue,
        };
        match hi {
            None => hi = Some(v),
            Some(h) => {
                out.push((h << 4) | v);
                hi = None;
            }
        }
    }
    out
}

/// 计算 SHA-256 摘要（复用 sha2 crate）。
fn sha256_digest(data: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().to_vec()
}

/// URL 解码：`%XX` → 字节，`+` → 空格。
fn urldecode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'+' {
            out.push(b' ');
            i += 1;
        } else if c == b'%' && i + 2 < b.len() {
            let h = (b[i + 1] as char).to_digit(16);
            let l = (b[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (h, l) {
                out.push((h * 16 + l) as u8);
                i += 3;
            } else {
                out.push(c);
                i += 1;
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 简易 shell 单引号包裹（含单引号时替换为 `'\"'\"'`）。
fn sh_quote(s: &str) -> String {
    if s.contains('\'') {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        format!("'{}'", s)
    }
}

/// 通配符匹配：`*` 匹配任意（含空）串，`?` 匹配单字符。
fn glob_match(text: &str, pat: &str) -> bool {
    fn m(t: &[u8], p: &[u8]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        match p[0] {
            b'*' => {
                let mut i = 1;
                while i < p.len() && p[i] == b'*' {
                    i += 1;
                }
                if i == p.len() {
                    return true;
                }
                for j in 0..=t.len() {
                    if m(&t[j..], &p[i..]) {
                        return true;
                    }
                }
                false
            }
            b'?' => !t.is_empty() && m(&t[1..], &p[1..]),
            c => !t.is_empty() && t[0] == c && m(&t[1..], &p[1..]),
        }
    }
    m(text.as_bytes(), pat.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct B {
        kv: HashMap<String, String>,
        args: HashMap<String, String>,
    }
    impl Builtin for B {
        fn cmd(&self, s: &str) -> String {
            format!("[cmd {}]", s)
        }
        fn fetch(&self, u: &str, _t: u64) -> String {
            format!("[fetch {}]", u)
        }
        fn kv_get(&self, k: &str) -> Option<String> {
            self.kv.get(k).cloned()
        }
        fn kv_set(&self, _k: &str, _v: &str) -> bool {
            true
        }
        fn arg(&self, k: &str) -> Option<String> {
            self.args.get(k).cloned()
        }
        fn has_arg(&self, k: &str) -> bool {
            self.args.contains_key(k)
        }
        fn post(&self, _u: &str, _b: &str) -> String {
            "[post]".into()
        }
        fn http_status(&self, _u: &str) -> String {
            "200".into()
        }
        fn read_file(&self, _p: &str) -> String {
            String::new()
        }
        fn write_file(&self, _p: &str, _c: &str) -> bool {
            true
        }
        fn append_file(&self, _p: &str, _c: &str) -> bool {
            true
        }
        fn ls(&self, _p: &str) -> String {
            String::new()
        }
        fn file_info(&self, _p: &str) -> String {
            "0;0;0".into()
        }
        fn lookup_ip(&self, _h: &str) -> String {
            String::new()
        }
        fn kill_pid(&self, _p: u32) -> bool {
            true
        }
        fn sha1(&self, _p: &str) -> String {
            "-".into()
        }
        fn urlenc(&self, _p: &str) -> String {
            "-".into()
        }
    }

    fn run(script: &str, args: HashMap<String, String>) -> (String, Vec<String>, Result<(), String>) {
        let b = B {
            kv: HashMap::new(),
            args,
        };
        let mut it = Interp::new(&b);
        match it.run(script) {
            Ok(o) => (o.value, o.logs, Ok(())),
            Err(e) => (String::new(), Vec::new(), Err(e)),
        }
    }

    #[test]
    fn arithmetic_compare() {
        let (v, _, r) = run("x = 2 + 3 * 4\nif x == 14\n  ret(\"yes\")\nelse\n  ret(\"no\")\nend", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "yes");
    }

    #[test]
    fn for_loop() {
        let (v, _, r) = run("s = 0\nfor i in range(5)\n  s = s + i\nend\nret(s)", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "10");
    }

    #[test]
    fn while_break() {
        let (v, _, r) = run("i = 0\nwhile i < 100\n  if i == 3\n    break\n  end\n  i = i + 1\nend\nret(i)", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "3");
    }

    #[test]
    fn string_concat_and_funcs() {
        let (v, _, r) = run("a = \"hello\"\nb = a + \" \" + upper(\"world\")\nret(b + \" \" + itoa(1+2))", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "hello WORLD 3");
    }

    #[test]
    fn args_and_log() {
        let (v, logs, r) = run("log(\"greet \" + arg(\"name\"))\nret(has_arg(\"name\"))", [("name".to_string(), "bob".to_string())].into_iter().collect());
        assert!(r.is_ok());
        assert_eq!(v, "true");
        assert!(logs[0].contains("greet bob"));
    }

    #[test]
    fn kv_prefix() {
        let b = B {
            kv: HashMap::new(),
            args: HashMap::new(),
        };
        let mut it = Interp::with_prefix("demo".to_string(), &b);
        let _ = it.run("kv_set(\"count\", \"1\")\nret(kv_get(\"count\"))");
        let w = it.take_kv_writes();
        assert_eq!(w, vec![("demo:count".to_string(), "1".to_string())]);
    }

    #[test]
    fn function_calls() {
        let (v, _, r) = run("ret(json(\"a\") )", HashMap::new());
        // json 包裹一层引号
        assert!(r.is_ok());
        assert_eq!(v, "\"a\"");
    }

    #[test]
    fn list_iteration() {
        let (v, _, r) = run(
            "all = split_list(\"a,b,c\", \",\")\nacc = \"\"\nfor x in all\n  acc = acc + x + \"|\"\nend\nret(acc)",
            HashMap::new(),
        );
        assert!(r.is_ok());
        assert_eq!(v, "a|b|c|");
    }

    #[test]
    fn list_funcs() {
        let (v, _, r) = run(
            "l = push(split_list(\"a,b\", \",\"), \"c\")\nret(at(l, 2) + \":\" + len(l))",
            HashMap::new(),
        );
        assert!(r.is_ok());
        assert_eq!(v, "c:3");
    }

    #[test]
    fn string_enhanced() {
        let (v, _, r) = run(
            "s = \"hello world\"\nret(itoa(contains(s, \"world\")) + \";\" + itoa(startswith(s, \"hel\")) + \";\" + count(s, \"l\"))",
            HashMap::new(),
        );
        assert!(r.is_ok());
        assert_eq!(v, "true;true;3");
    }

    #[test]
    fn range_rand_now() {
        let (_v, _, r) = run("ret(len(range(3)))", HashMap::new());
        // range 返回数值型，实际上 range 被当作数值。验证 rand/now 为数值
        assert!(r.is_ok());
        let (v2, _, r2) = run("a = now()\nb = a + 1\nret(b > a)", HashMap::new());
        assert!(r2.is_ok());
        assert_eq!(v2, "true");
        // rand(0,0) 返回 0
        let (v3, _, r3) = run("ret(rand(5,5))", HashMap::new());
        assert!(r3.is_ok());
        assert_eq!(v3, "5");
    }

    #[test]
    fn new_helpers_json() {
        let (v, _, r) = run(
            "j = \"{\\\"a\\\":1,\\\"b\\\":\\\"hi\\\",\\\"n\\\":[\\\"x\\\",\\\"y\\\"]}\"\nret(json_get(j, \"b\"))",
            HashMap::new(),
        );
        assert!(r.is_ok());
        assert_eq!(v, "hi");
        // 数组下标
        let (v2, _, r2) = run(
            "j = \"{\\\"n\\\":[\\\"x\\\",\\\"y\\\"]}\"\nret(json_get(j, \"n[1]\"))",
            HashMap::new(),
        );
        assert!(r2.is_ok());
        assert_eq!(v2, "y");
        // numeric
        let (v3, _, r3) = run(
            "j = \"{\\\"a\\\":1,\\\"b\\\":\\\"c\\\"}\"\nret(json_get(j, \"a\"))",
            HashMap::new(),
        );
        assert!(r3.is_ok());
        assert_eq!(v3, "1");
    }

    #[test]
    fn new_helpers_keys_fmt() {
        let (v, _, r) = run(
            "j = \"{\\\"a\\\":1,\\\"b\\\":2}\"\nks = keys(j)\nret(join(ks, \"|\"))",
            HashMap::new(),
        );
        assert!(r.is_ok());
        assert_eq!(v, "a|b");
        let (v2, _, r2) = run("ret(fmt_bytes(1536))", HashMap::new());
        assert!(r2.is_ok());
        assert!(v2.ends_with('K'));
        let (v3, _, r3) = run("ret(fmt_dur(3661))", HashMap::new());
        assert!(r3.is_ok());
        assert!(v3.contains('h') && v3.contains('m'));
    }

    #[test]
    fn new_helpers_hash_path_stats() {
        // sha256 为 64 位小写 hex
        let (v, _, r) = run("ret(sha256(\"abc\"))", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        // urldec
        let (v, _, r) = run("ret(urldec(\"a+b%20c\"))", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "a b c");
        // 路径
        let (v, _, r) = run("ret(basename(\"/etc/nginx.conf\") + \";\" + dirname(\"/etc/nginx.conf\") + \";\" + extname(\"/a/b.tar.gz\"))", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "nginx.conf;/etc;gz");
        // 统计
        let (v, _, r) = run("n = lines(\"1\\n2\\n3\")\nret(itoa(sum(n)) + \";\" + itoa(avg(n)))", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "6;2");
        // 百分比与 UUID 长度
        let (v, _, r) = run("ret(pct(25, 100))", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "25");
        let (v, _, r) = run("ret(len(uuid()))", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "36");
        // hex 编解码
        let (v, _, r) = run("ret(hexdec(hexenc(\"hi\")))", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "hi");
    }

    #[test]
    fn new_helpers_b64_glob() {
        let (v, _, r) = run("ret(b64(\"hello\"))", HashMap::new());
        assert!(r.is_ok());
        assert_eq!(v, "aGVsbG8=");
        let (v2, _, r2) = run("ret(b64d(\"aGVsbG8=\"))", HashMap::new());
        assert!(r2.is_ok());
        assert_eq!(v2, "hello");
        let (v3, _, r3) = run("ret(glob(\"access.log\", \"access.*\"))", HashMap::new());
        assert!(r3.is_ok());
        assert_eq!(v3, "true");
    }

    #[test]
    fn new_helpers_list() {
        let (v, _, r) = run(
            "l = uniq(split_list(\"a,b,a,c\", \",\"))\nret(join(sortlist(l), \"\"))",
            HashMap::new(),
        );
        assert!(r.is_ok());
        assert_eq!(v, "abc");
        let (v3, _, r3) = run("ret(len(words(\"x y z\")))", HashMap::new());
        assert!(r3.is_ok());
        assert_eq!(v3, "3");
        let (v4, _, r4) = run("ret(repeat(\"ab\", 3))", HashMap::new());
        assert!(r4.is_ok());
        assert_eq!(v4, "ababab");
    }
}