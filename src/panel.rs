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
</div>

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