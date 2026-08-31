//! 登录认证与账户安全：PBKDF1-SHA1 密码哈希、HMAC-SHA1 签名会话、
//! 失败锁定、会话管理、修改密码与初始设置向导。
//!
//! 一切用内存里的有界结构控制，密码哈希 / 签名只用 `sha1` crate，不引入重型依赖，
//! 保持 vPanel 的低常驻内存。状态落盘到 `<panel_dir>/.vpanel-auth.json`。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use sha1::{Digest, Sha1};

use crate::config::Security;
use crate::json;

/// 登录结果。
#[derive(Debug, Clone, PartialEq)]
pub enum Login {
    Ok,
    Bad,
    /// 已锁定，给出剩余锁定秒数。
    Locked(i64),
}

/// 一次登录的完整结果：类型 + 成功时的新 cookie。
#[derive(Debug, Clone)]
pub struct LoginOutcome {
    pub kind: Login,
    pub cookie: Option<String>,
    pub exp: i64,
}

/// 持久化状态（写入 `.vpanel-auth.json`）。
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Persist {
    /// 面板签名密钥（十六进制），首次运行随机生成。
    secret: String,
    /// 密码哈希：`salt$rounds$hex`。
    pw: String,
    /// 会话：sid -> 描述。
    sessions: HashMap<String, Ses>,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
struct Ses {
    exp: i64,
    last: i64,
    ua: String,
}

/// 运行态（含内存中的失败计数，不落盘）。
struct Inner {
    persist: Persist,
    /// 客户端键 -> (失败次数, 锁定截止 epoch)
    fails: HashMap<String, (u32, i64)>,
}

/// 对外安全门面（挂到 State）。
pub struct SecurityGuard {
    cfg: Security,
    inner: Mutex<Inner>,
}

const ROUNDS: u32 = 16000;
const AUTH_PATH: &str = ".vpanel-auth.json";

impl SecurityGuard {
    pub fn new(cfg: Security) -> SecurityGuard {
        let mut persist = load();
        if persist.secret.is_empty() {
            persist.secret = hex(rand_bytes(32));
        }
        let g = SecurityGuard {
            inner: Mutex::new(Inner {
                persist,
                fails: HashMap::new(),
            }),
            cfg,
        };
        // 配置明文密码：仅当尚无哈希时作为一次性初始密码写入。
        if !g.cfg.password.is_empty() && g.inner.lock().unwrap().persist.pw.is_empty() {
            g.set_password(&g.cfg.password);
        }
        g
    }

    /// 认证是否开启。
    pub fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    /// 是否已设置密码（区分登录 vs 初始向导）。
    pub fn has_password(&self) -> bool {
        !self.inner.lock().unwrap().persist.pw.is_empty()
    }

    /// 处于初始设置态：开了认证但还没设密码。
    pub fn needs_setup(&self) -> bool {
        self.cfg.enabled && !self.has_password()
    }

    pub fn session_count(&self) -> usize {
        self.inner.lock().unwrap().persist.sessions.len()
    }

    /// 校验 cookie 是否有效（签名 + 未过期 + 会话存在）。
    pub fn validate(&self, cookie: Option<&str>) -> bool {
        let Some(cv) = cookie else { return false };
        let parts: Vec<&str> = cv.split('.').collect();
        if parts.len() != 3 {
            return false;
        }
        let (sid, exp_s, mac_s) = (parts[0], parts[1], parts[2]);
        let exp: i64 = match exp_s.parse() {
            Ok(e) => e,
            Err(_) => return false,
        };
        let inner = self.inner.lock().unwrap();
        if now() > exp {
            return false;
        }
        if !ct_eq(&mac(&inner.persist.secret, sid, exp), &mac_s) {
            return false;
        }
        if !inner.persist.sessions.contains_key(sid) {
            return false;
        }
        true
    }

    /// 登录（会话级，不记住我）。
    pub fn login(&self, pw: &str, client_key: &str, ua: &str) -> LoginOutcome {
        self.login_full(pw, client_key, ua, false)
    }

    /// 登录。`remember` 为真时有效期取 `remember_days`，否则 `session_hours`；
    /// 开启 `single_session` 时新登录自动踢出旧会话，保证单账号单会话。
    pub fn login_full(&self, pw: &str, client_key: &str, ua: &str, remember: bool) -> LoginOutcome {
        // 锁定判断
        {
            let inner = self.inner.lock().unwrap();
            if let Some((_c, u)) = inner.fails.get(client_key) {
                if now() < *u {
                    return LoginOutcome {
                        kind: Login::Locked(u - now()),
                        cookie: None,
                        exp: 0,
                    };
                }
            }
        }
        let ok = {
            let inner = self.inner.lock().unwrap();
            verify(&inner.persist.pw, pw)
        };
        if !ok {
            let (c, until) = self.bump_fail(client_key);
            if c >= self.cfg.max_failures {
                return LoginOutcome {
                    kind: Login::Locked(until - now()),
                    cookie: None,
                    exp: 0,
                };
            }
            return LoginOutcome {
                kind: Login::Bad,
                cookie: None,
                exp: 0,
            };
        }
        self.inner.lock().unwrap().fails.remove(client_key);
        // 单账号单会话：踢出旧会话。
        if self.cfg.single_session {
            self.inner.lock().unwrap().persist.sessions.clear();
            save(&self.inner.lock().unwrap().persist);
        }
        let ttl = self.ttl(remember);
        let cookie = self.mint_session_for(ua, ttl);
        LoginOutcome {
            kind: Login::Ok,
            cookie: Some(cookie),
            exp: now() + ttl,
        }
    }

    /// 登出一个或所有会话。
    pub fn logout(&self, cookie: Option<&str>, all: bool) {
        let mut inner = self.inner.lock().unwrap();
        if all {
            inner.persist.sessions.clear();
        } else if let Some(cv) = cookie {
            let sid = cv.split('.').next().unwrap_or("").to_string();
            inner.persist.sessions.remove(&sid);
        }
        save(&inner.persist);
    }

    /// 修改密码。旧密码正确且新密码合法才允许。
    /// 保留当前会话（保持登录态），清除并强制下线所有其它会话，返回当前会话的 cookie。
    pub fn change_password(&self, cookie: Option<&str>, old: &str, new: &str, _ua: &str) -> Option<String> {
        if new.len() < 8 {
            return None;
        }
        {
            let inner = self.inner.lock().unwrap();
            if !verify(&inner.persist.pw, old) {
                return None;
            }
        }
        // 当前会话对应的 sid（cookie 需本身有效）。
        let cur_sid = cookie.and_then(|c| {
            let v: Vec<&str> = c.split('.').collect();
            if v.len() == 3 && self.validate(Some(c)) {
                Some(v[0].to_string())
            } else {
                None
            }
        });
        let mut inner = self.inner.lock().unwrap();
        inner.persist.pw = hash_pw(new);
        // 保留当前会话，清除其它。
        if let Some(sid) = &cur_sid {
            let keep = inner.persist.sessions.remove(sid);
            inner.persist.sessions.clear();
            if let Some(s) = keep {
                inner.persist.sessions.insert(sid.clone(), s);
            }
        } else {
            inner.persist.sessions.clear();
        }
        save(&inner.persist);
        drop(inner);
        cookie.map(|s| s.to_string())
    }

    /// 会话列表 JSON。
    pub fn sessions_json(&self) -> String {
        let inner = self.inner.lock().unwrap();
        let t = now();
        let arr: Vec<String> = inner
            .persist
            .sessions
            .iter()
            .map(|(sid, s)| {
                format!(
                    "{{\"sid\":\"{}\",\"ua\":\"{}\",\"expires_in\":{}}}",
                    json::jesc(sid),
                    json::jesc(&s.ua),
                    (s.exp - t).max(0)
                )
            })
            .collect();
        format!("{{\"ok\":true,\"sessions\":[{}]}}", arr.join(","))
    }

    /// 强制下线某会话。
    pub fn revoke(&self, sid: &str) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let r = inner.persist.sessions.remove(sid).is_some();
        if r {
            save(&inner.persist);
        }
        r
    }

    /// 初始设置：设置管理员密码并登录，返回新 cookie。
    pub fn setup(&self, pw: &str, ua: &str) -> Option<String> {
        self.setup_full(pw, ua, false)
    }

    /// 初始设置（可勾选记住我）。
    pub fn setup_full(&self, pw: &str, ua: &str, remember: bool) -> Option<String> {
        if self.has_password() {
            return None;
        }
        if pw.len() < 8 {
            return None;
        }
        self.set_password(pw);
        Some(self.mint_session_for(ua, self.ttl(remember)))
    }

    /// 会话有效期（秒），记住我则取 remember_days。
    fn ttl(&self, remember: bool) -> i64 {
        if remember && self.cfg.remember_days > 0 {
            (self.cfg.remember_days as i64) * 86400
        } else {
            (self.cfg.session_hours as i64) * 3600
        }
    }

    /// 写密码（不改动已有哈希的语义由调用方保证）。
    fn set_password(&self, pw: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.persist.pw = hash_pw(pw);
        inner.persist.sessions.clear();
        save(&inner.persist);
    }

    /// 生成新会话并签出 cookie 值（登记到会话表 + 落盘）。
    fn mint_session(&self, ua: &str) -> String {
        self.mint_session_for(ua, (self.cfg.session_hours as i64) * 3600)
    }

    /// 以指定有效期生成会话。
    fn mint_session_for(&self, ua: &str, ttl: i64) -> String {
        let sid = hex(rand_bytes(16));
        let exp = now() + ttl;
        let mut inner = self.inner.lock().unwrap();
        inner.persist.sessions.insert(
            sid.clone(),
            Ses {
                exp,
                last: now(),
                ua: ua.to_string(),
            },
        );
        save(&inner.persist);
        format!("{}.{}.{}", sid, exp, mac(&inner.persist.secret, &sid, exp))
    }

    fn bump_fail(&self, key: &str) -> (u32, i64) {
        let mut inner = self.inner.lock().unwrap();
        let (c, _) = inner.fails.get(key).copied().unwrap_or((0, 0));
        let c = c + 1;
        if c >= self.cfg.max_failures {
            let until = now() + (self.cfg.lock_minutes as i64) * 60;
            inner.fails.insert(key.to_string(), (c, until));
            (c, until)
        } else {
            inner.fails.insert(key.to_string(), (c, 0));
            (c, 0)
        }
    }
}

// ---------------------------------------------------------------------------
// 密码学工具
// ---------------------------------------------------------------------------

/// PBKDF1 风格哈希：迭代 SHA1(盐 || 密码)。返回 `salt$rounds$hex`。
fn hash_pw(pw: &str) -> String {
    let salt = hex(rand_bytes(12));
    let mut d = salt.as_bytes().to_vec();
    d.extend_from_slice(pw.as_bytes());
    let mut out = Sha1::digest(&d);
    for _ in 1..ROUNDS {
        out = Sha1::digest(out);
    }
    format!("{}${}${}", salt, ROUNDS, fmt_hex(&out))
}

/// 校验 `salt$rounds$hex` 是否匹配明文。
fn verify(stored: &str, pw: &str) -> bool {
    let mut it = stored.trim().split('$');
    let salt = match it.next() {
        Some(s) if !s.is_empty() => s,
        _ => return false,
    };
    let rounds = match it.next().and_then(|r| r.parse::<u32>().ok()) {
        Some(r) if r > 0 => r,
        _ => return false,
    };
    let want = match it.next() {
        Some(h) if !h.is_empty() => h,
        _ => return false,
    };
    let mut d = salt.as_bytes().to_vec();
    d.extend_from_slice(pw.as_bytes());
    let mut out = Sha1::digest(&d);
    for _ in 1..rounds {
        out = Sha1::digest(out);
    }
    let got = fmt_hex(&out);
    // 常量时间比较
    ct_eq(&got, &want)
}

/// 常量时间字符串比较，杜绝时序侧信道。
fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// HMAC-SHA1（RFC 2104）。
fn mac(secret_hex: &str, sid: &str, exp: i64) -> String {
    let key = from_hex(secret_hex);
    let msg = format!("vs:{}:{}", sid, exp);
    let mut ik = [0u8; 64];
    let mut ok = [0u8; 64];
    for i in 0..64 {
        ik[i] = key.get(i).copied().unwrap_or(0) ^ 0x36;
        ok[i] = key.get(i).copied().unwrap_or(0) ^ 0x5c;
    }
    let mut inner_buf = Vec::with_capacity(64 + msg.len());
    inner_buf.extend_from_slice(&ik);
    inner_buf.extend_from_slice(msg.as_bytes());
    let inner = Sha1::digest(&inner_buf);

    let mut outer_buf = Vec::with_capacity(64 + 20);
    outer_buf.extend_from_slice(&ok);
    outer_buf.extend_from_slice(&inner);
    let outer = Sha1::digest(&outer_buf);
    fmt_hex(&outer)
}

// ---------------------------------------------------------------------------
// 熵与时间
// ---------------------------------------------------------------------------

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn rand_bytes(n: usize) -> Vec<u8> {
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let mut buf = vec![0u8; n];
        if f.read_exact(&mut buf).is_ok() {
            return buf;
        }
    }
    let mut seed = seed();
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        out.push((seed >> 33) as u8);
    }
    out
}

fn seed() -> u64 {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let pid = std::process::id() as u64;
    let addr = std::ptr::null::<u8>() as usize as u64;
    time ^ pid.wrapping_mul(2654435761) ^ addr.wrapping_shl(32)
}

fn hex(b: Vec<u8>) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn fmt_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn from_hex(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len() / 2);
    let mut i = 0;
    while i + 1 < b.len() {
        if let (Some(h), Some(l)) = ((b[i] as char).to_digit(16), (b[i + 1] as char).to_digit(16)) {
            out.push((h * 16 + l) as u8);
        }
        i += 2;
    }
    out
}

// ---------------------------------------------------------------------------
// 持久化
// ---------------------------------------------------------------------------

fn auth_path() -> String {
    format!("{}/{}", crate::config::Config::panel_dir(), AUTH_PATH)
}

fn load() -> Persist {
    if let Ok(s) = std::fs::read_to_string(auth_path()) {
        if let Ok(p) = serde_json::from_str::<Persist>(&s) {
            return p;
        }
    }
    Persist::default()
}

fn save(p: &Persist) {
    if let Ok(s) = serde_json::to_string(p) {
        write_private(auth_path(), s.as_bytes());
    }
}

/// 以 0600 权限写入敏感状态文件，避免其它本地用户读到 HMAC secret 与会话。
fn write_private(path: String, data: &[u8]) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
        {
            let _ = f.write_all(data);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::write(&path, data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个用例串行执行，避免并行时 VPVPANEL_DIR（进程级全局环境变量）互相覆盖。
    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
        L.lock().unwrap()
    }

    fn guard() -> SecurityGuard {
        // 用独立临时目录避免污染真实 auth.json；每个用例唯一，避免跨用例残留。
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("vpanel_auth_test_{}_{}", std::process::id(), seq));
        std::env::set_var("VPVPANEL_DIR", &dir);
        let _ = std::fs::create_dir_all(&dir);
        // 清理可能由进程 PID 复用残留的历史状态，保证每次都从零开始。
        let _ = std::fs::remove_file(dir.join(AUTH_PATH));
        let cfg = Security {
            enabled: true,
            password: String::new(),
            mcp_token: String::new(),
            max_failures: 3,
            lock_minutes: 5,
            session_hours: 24,
            remember_days: 30,
            single_session: false, // 测试需要多会话共存
            trust_proxy: false,
        };
        SecurityGuard::new(cfg)
    }

    fn setup_guard(single: bool, remember: u32, hours: u32) -> (std::path::PathBuf, Security) {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("vpanel_auth_sg_{}_{}", std::process::id(), seq));
        let _ = std::fs::create_dir_all(&dir);
        // 清理可能由进程 PID 复用残留的历史状态。
        let _ = std::fs::remove_file(dir.join(AUTH_PATH));
        let cfg = Security {
            enabled: true,
            password: String::new(),
            mcp_token: String::new(),
            max_failures: 3,
            lock_minutes: 5,
            session_hours: hours,
            remember_days: remember,
            single_session: single,
            trust_proxy: false,
        };
        (dir, cfg)
    }

    #[test]
    fn setup_then_login() {
        let _lk = test_lock();
        let g = guard();
        assert!(g.needs_setup());
        assert!(!g.has_password());
        assert!(g.setup("secret123", "test-ua").is_some());
        assert!(g.has_password());
        assert!(!g.needs_setup());
        // 正确密码
        let out = g.login("secret123", "ip1", "ua");
        assert_eq!(out.kind, Login::Ok);
        let c = out.cookie.unwrap();
        assert!(g.validate(Some(&c)));
        // 错误密码
        let out = g.login("wrong", "ip1", "ua");
        assert_eq!(out.kind, Login::Bad);
        // 篡改 cookie 不通过
        let tampered = format!("{}x", c);
        assert!(!g.validate(Some(&tampered)));
    }

    #[test]
    fn lockout_after_failures() {
        let _lk = test_lock();
        let g = guard();
        g.setup("secret123", "ua").unwrap();
        assert!(matches!(g.login("bad", "ip", "ua").kind, Login::Bad));
        assert!(matches!(g.login("bad", "ip", "ua").kind, Login::Bad));
        assert!(matches!(g.login("bad", "ip", "ua").kind, Login::Locked(_)));
        // 锁定期间即使密码正确也拒绝
        assert!(matches!(g.login("secret123", "ip", "ua").kind, Login::Locked(_)));
    }

    #[test]
    fn change_password_invalidates_others() {
        let _lk = test_lock();
        let g = guard();
        // 初始设置向导会自动登录，其 cookie 即「当前会话」。
        let a = g.setup("secret123", "ua").unwrap();
        let b = g.login("secret123", "ip", "ua").cookie.unwrap();
        assert!(g.validate(Some(&a)));
        assert!(g.validate(Some(&b)));
        assert_eq!(g.session_count(), 2);
        // 旧密码错则拒绝
        assert!(g.change_password(Some(&a), "wrong", "newpw123", "ua").is_none());
        // 正确修改：保留当前会话，清除其它
        let newc = g.change_password(Some(&a), "secret123", "newpw123", "ua").unwrap();
        assert!(g.validate(Some(&newc)));
        assert!(g.validate(Some(&a)));
        assert!(!g.validate(Some(&b)));
        assert_eq!(g.session_count(), 1);
    }

    #[test]
    fn revoke_session() {
        let _lk = test_lock();
        let g = guard();
        g.setup("secret123", "ua").unwrap();
        let a = g.login("secret123", "ip", "ua").cookie.unwrap();
        let sid = a.split('.').next().unwrap().to_string();
        assert!(g.revoke(&sid));
        assert!(!g.validate(Some(&a)));
    }

    #[test]
    fn single_session_evicts_old() {
        let _lk = test_lock();
        let (dir, cfg) = setup_guard(true, 30, 2);
        std::env::set_var("VPVPANEL_DIR", &dir);
        let g = SecurityGuard::new(cfg);
        g.setup("secret123", "ua").unwrap();
        let a = g.login("secret123", "ip", "ua").cookie.unwrap();
        let b = g.login("secret123", "ip2", "ua").cookie.unwrap();
        assert!(g.validate(Some(&b)));
        assert!(!g.validate(Some(&a)), "首会话应被新登录踢出");
        assert_eq!(g.session_count(), 1);
    }

    #[test]
    fn remember_extends_ttl() {
        // remember_days=30 远大于 session_hours=24，应得到更长有效期。
        let _lk = test_lock();
        let (dir, cfg) = setup_guard(false, 30, 24);
        std::env::set_var("VPVPANEL_DIR", &dir);
        let g = SecurityGuard::new(cfg);
        g.setup("secret123", "ua").unwrap();
        let normal = g.login("secret123", "ip", "ua");
        let rem = g.login_full("secret123", "ip2", "ua", true);
        let (n_exp, r_exp) = (normal.exp, rem.exp);
        assert!(r_exp > n_exp, "记住我会话应更持久 r={} n={}", r_exp, n_exp);
    }
}