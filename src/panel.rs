//! 清亮面板的 HTML 渲染与运行时指标。
//!
//! 页面完全内联（CSS/JS 都写在页面里），不依赖外部资源，保证干净、即时加载。

use crate::http::State;

/// 读取本进程常驻内存（VmRSS），单位 MB（保留 2 位小数）。
pub fn rss_mb() -> f64 {
    rss_kb() as f64 / 1024.0
}

/// 读取本进程常驻内存（VmRSS）KB。失败返回 0。
pub fn rss_kb() -> u64 {
    if let Ok(s) = std::fs::read_to_string("/proc/self/status") {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                if let Ok(v) = rest.trim().trim_end_matches("kB").trim().parse::<u64>() {
                    return v;
                }
            }
        }
    }
    0
}

/// 渲染面板首页。
pub fn render(state: &State) -> String {
    let cfg = &state.cfg;
    let up = state.started.elapsed().as_secs();
    let uptime = fmt_duration(up);
    let rss = rss_mb();
    let req = state.requests.load(std::sync::atomic::Ordering::Relaxed);
    let act = state.active.load(std::sync::atomic::Ordering::Relaxed);
    let conns = state.conns.load(std::sync::atomic::Ordering::Relaxed);
    let accent = esc(&cfg.panel.accent);
    let dark = cfg.panel.theme.eq_ignore_ascii_case("dark");

    // 请求配额卡：占用内存（MB），目标 ~2 (常驻) / <=3 (高并发)。
    let idle_badge = if rss <= 2.2 { "达标" } else { "超标" };

    format!(r###"<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · {subtitle}</title>
<style>
:root{{--bg:#f4f6fb;--card:#ffffff;--ink:#1f2937;--muted:#6b7280;--line:#e5e7eb;--accent:{accent};--shadow:0 10px 30px rgba(17,24,39,.08)}}
*{{box-sizing:border-box;margin:0;padding:0}}
body{{font-family:system-ui,-apple-system,"PingFang SC","Microsoft YaHei",sans-serif;background:var(--bg);color:var(--ink);min-height:100vh;}} {dark_mode}
.wrap{{max-width:880px;margin:0 auto;padding:56px 24px 80px}}
header{{display:flex;align-items:baseline;justify-content:space-between;border-bottom:2px solid var(--accent);padding-bottom:16px;margin-bottom:28px}}
h1{{font-size:30px;letter-spacing:.5px;font-weight:800}} h1 span{{color:var(--accent)}}
.tag{{color:var(--muted);font-size:13px}}
.grid{{display:grid;grid-template-columns:repeat(4,1fr);gap:16px;margin-bottom:28px}}
.card{{background:var(--card);border:1px solid var(--line);border-radius:16px;box-shadow:var(--shadow);padding:20px}}
.card .l{{color:var(--muted);font-size:12px;letter-spacing:1px;text-transform:uppercase;margin-bottom:8px}}
.card .v{{font-size:28px;font-weight:800;font-variant-numeric:tabular-nums}}
.card .s{{color:var(--muted);font-size:12px;margin-top:6px}}
.badge{{display:inline-block;font-size:11px;font-weight:700;padding:2px 8px;border-radius:999px;margin-left:6px;vertical-align:middle}}
.ok{{background:#dcfce7;color:#166534}} .bad{{background:#fee2e2;color:#991b1b}}
.row{{display:flex;justify-content:space-between;padding:12px 0;border-bottom:1px solid var(--line);font-size:14px}}
.row:last-child{{border-bottom:none}} .row b{{font-weight:600}}
.k{{color:var(--muted)}}
.btn{{display:inline-flex;align-items:center;gap:8px;background:var(--accent);color:#fff;text-decoration:none;font-weight:700;padding:12px 20px;border-radius:12px;font-size:14px;box-shadow:0 6px 16px rgba(37,99,235,.25);transition:transform .1s}}
.btn:hover{{transform:translateY(-1px)}}
.mem{{display:flex;align-items:center;gap:14px}}
.bar{{flex:1;height:8px;background:#eef2f7;border-radius:999px;overflow:hidden}}
.bar i{{display:block;height:100%;background:var(--accent);border-radius:999px;transition:width .4s}}
footer{{text-align:center;color:var(--muted);font-size:12px;margin-top:40px}}
@media(max-width:640px){{.grid{{grid-template-columns:repeat(2,1fr)}}}}
</style>
</head>
<body>
<div class="wrap">
<header>
  <h1><span>{title}</span></h1>
  <div class="tag">{subtitle}</div>
</header>

<div class="grid">
  <div class="card"><div class="l">常驻内存</div><div class="v">{rss:.2}<small style="font-size:14px">MB</small></div><div class="s">目标 ≈ 2 MB<span class="{idle_cls}">{idle_badge}</span></div></div>
  <div class="card"><div class="l">请求总数</div><div class="v">{req}</div><div class="s">已处理请求</div></div>
  <div class="card"><div class="l">并发连接</div><div class="v">{act}</div><div class="s">当前活跃</div></div>
  <div class="card"><div class="l">运行时长</div><div class="v">{uptime}</div><div class="s">累计连接 {conns}</div></div>
</div>

<div class="card" style="margin-bottom:28px">
  <div class="l" style="margin-bottom:14px">内存水位 <small style="text-transform:none">(高并发限 ≤ 3 MB)</small></div>
  <div class="mem">
    <div class="bar"><i style="width:min(100%,{bar_pct:.0}%)"></i></div>
    <b>{rss:.2} MB</b>
  </div>
</div>

<div class="card">
  <div class="l" style="margin-bottom:6px">服务信息</div>
  <div class="row"><span class="k">监听地址</span><b>{bind}:{port}</b></div>
  <div class="row"><span class="k">工作线程</span><b>{workers}（固定）</b></div>
  <div class="row"><span class="k">队列上限</span><b>{backlog}</b></div>
  <div class="row"><span class="k">主题</span><b>{theme}</b></div>
  <div class="row"><span class="k">运行环境</span><b>{arch}/{os}</b></div>
  <div class="row"><span class="k">Web 终端</span><b>{shell_state}</b></div>
</div>

{shell_card}
<footer>Lumen Panel · zero-runtime HTTP dashboard · 内存恒定</footer>
</div>
</body>
</html>"###,
        title = esc(&cfg.panel.title),
        subtitle = esc(&cfg.panel.subtitle),
        bind = esc(&cfg.server.bind),
        port = cfg.server.port,
        workers = cfg.server.workers,
        backlog = cfg.server.backlog,
        uptime = uptime,
        rss = rss,
        req = req,
        act = act,
        conns = conns,
        bar_pct = (rss / 3.0 * 100.0).clamp(0.0, 100.0),
        idle_badge = idle_badge,
        idle_cls = if rss <= 2.2 { "badge ok" } else { "badge bad" },
        theme = esc(&cfg.panel.theme),
        arch = std::env::consts::ARCH,
        os = std::env::consts::OS,
        shell_state = if cfg.shell.enabled { "已启用" } else { "已禁用" },
        shell_card = shell_card(cfg),
        dark_mode = if dark {
            ":root{--bg:#0f172a;--card:#1e293b;--ink:#e2e8f0;--muted:#94a3b8;--line:#334155;--shadow:0 10px 30px rgba(0,0,0,.4)}"
        } else { "" },
    )
}

/// 简单的 HTML 转义，避免配置内容破坏页面结构。
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// 终端入口卡片（在终端启用时显示通往 /term 的按钮）。
fn shell_card(cfg: &crate::config::Config) -> String {
    if !cfg.shell.enabled {
        return String::new();
    }
    format!(
        "<div class=\"card\" style=\"margin-bottom:28px\"><div class=\"l\" style=\"margin-bottom:14px\">Web 终端</div><div style=\"display:flex;align-items:center;justify-content:space-between\"><div style=\"font-size:14px\">在浏览器里直接操控本机 Shell（{cmd}），无需 SSH 公钥。\n</div><a class=\"btn loadterm\" href=\"/term\" style=\"flex:none\">&#9654; 打开终端</a></div></div>",
        cmd = cfg.shell.cmd
    )
}

/// Web 终端页面（基于 xterm.js + WebSocket）。
pub fn render_term(cfg: &crate::config::Config) -> String {
    let accent = esc(&cfg.panel.accent);
    let dark = cfg.panel.theme.eq_ignore_ascii_case("dark");
    let theme_css = if dark {
        "--bg:#0b1220;--line:#1e293b;"
    } else {
        "--bg:#ffffff;--line:#e5e7eb;"
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>终端 · {title}</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.5.0/css/xterm.min.css">
<style>
*{{box-sizing:border-box}}
body{{margin:0;padding:0;background:{bg};font-family:system-ui,sans-serif;height:100vh;display:flex;flex-direction:column}}
.top{{display:flex;align-items:center;gap:12px;padding:14px 18px;border-bottom:1px solid {line};background:{bg};color:#1f2937}}
.top .mk{{width:46px;height:46px;border-radius:12px;background:{accent};color:#fff;display:flex;align-items:center;justify-content:center;font-weight:800;font-size:22px}}
.top h1{{font-size:16px;margin:0;font-weight:800}}
.top .hint{{font-size:12px;color:#6b7280}}
.back{{margin-left:auto;color:{accent};text-decoration:none;font-size:13px;font-weight:700}}
#term{{flex:1;padding:6px 0}}
.status{{height:28px;font-size:11px;color:#6b7280;display:flex;align-items:center;padding:0 18px;border-top:1px solid {line}}}
</style>
</head>
<body>
<div class="top">
  <div class="mk">&#9654;</div>
  <div><h1>{title} · Web 终端</h1><div class="hint">{cmd}</div></div>
  <a class="back" href="/">&#8592; 返回面板</a>
</div>
<div id="term"></div>
<div class="status"><span id="st">连接中…</span></div>
<script src="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.5.0/lib/xterm.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0.10.0/lib/addon-fit.min.js"></script>
<script>
(function(){{
  const term = new Terminal({{cursorBlink:true, fontSize:14, fontFamily:'Menlo,Consolas,"Courier New",monospace'}});
  const fit = new FitAddon.FitAddon();
  term.loadAddon(fit);
  term.open(document.getElementById('term'));
  fit.fit();

  const proto = location.protocol === 'https:' ? 'wss://' : 'ws://';
  const ws = new WebSocket(proto + location.host + '/ws');
  const st = document.getElementById('st');

  ws.onopen = function(){{ st.textContent = '已连接 · ' + term.cols + 'x' + term.rows; term.focus(); }};
  ws.onclose = function(){{ st.textContent = '已断开'; term.dispose(); }};
  ws.onmessage = function(ev){{ term.write(ev.data); }};

  const enc = new TextEncoder();
  term.onData(function(d){{ ws.send(enc.encode(d)); }});

  const sendSize = function(){{ ws.send('st'+String.fromCharCode(9)+term.cols+String.fromCharCode(9)+term.rows); }};
  term.onResize(function(){{ sendSize(); }});
  sendSize();

  window.addEventListener('resize', function(){{ fit.fit(); sendSize(); }});
}})();
</script>
</body>
</html>"#,
        title = esc(&cfg.panel.title),
        cmd = esc(&cfg.shell.cmd),
        accent = accent,
        bg = theme_css,
        line = if dark { "#1e293b" } else { "#e5e7eb" },
    )
}

/// 秒数 -> 人类可读时长。
fn fmt_duration(total: u64) -> String {
    let d = total / 86400;
    let h = (total % 86400) / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if d > 0 {
        format!("{}d {}h {}m", d, h, m)
    } else if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}