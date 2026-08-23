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

// ---------------------------------------------------------------------------
// 外部能力接口（由 plugins 实现）
// ---------------------------------------------------------------------------

pub trait Builtin {
    fn cmd(&self, shell: &str) -> String;
    fn fetch(&self, url: &str, timeout: u64) -> String;
    fn kv_get(&self, key: &str) -> Option<String>;
    fn kv_set(&self, key: &str, val: &str) -> bool;
    fn arg(&self, name: &str) -> Option<String>;
    fn has_arg(&self, name: &str) -> bool;
}

// ---------------------------------------------------------------------------
// 值
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum V {
    S(String),
    N(f64),
    B(bool),
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
            V::Nil => String::new(),
        }
    }
    fn as_num(&self) -> f64 {
        match self {
            V::N(n) => *n,
            V::S(s) => s.trim().parse().unwrap_or(0.0),
            V::B(b) => if *b { 1.0 } else { 0.0 },
            V::Nil => 0.0,
        }
    }
    fn truthy(&self) -> bool {
        match self {
            V::B(b) => *b,
            V::N(n) => *n != 0.0,
            V::S(s) => !s.is_empty(),
            V::Nil => false,
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
            if !p.eat_ident("in") || !p.eat_ident("range") || !p.eat_op("(") {
                return Err("for 仅支持 for x in range(n)".into());
            }
            let n = p.expr()?;
            if !p.eat_op(")") {
                return Err("for range 缺右括号".into());
            }
            *pos += 1;
            let body = parse_block(lines, pos, header_indent as i64)?;
            Ok(Stmt::For { var, n, body })
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
                    let cnt = self.eval(n)?.as_num();
                    let cnt = cnt as i64;
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
            "len" => Ok(V::N(s(&one).chars().count() as f64)),
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
}