//! 面板前端：一个多标签的管理单页（概览/进程/服务/安全/定时任务 + 终端）。
//! 页面完全内联（CSS/JS 全写在页面里），数据通过 /api/* 异步拉取，零外部资源。

use crate::http::State;

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

/// 渲染面板首页（多标签管理控制台）。
pub fn render(state: &State) -> String {
    let cfg = &state.cfg;
    let accent = esc_attr(&cfg.panel.accent);
    let title = esc_attr(&cfg.panel.title);
    let subtitle = esc_attr(&cfg.panel.subtitle);
    let dark = cfg.panel.theme.eq_ignore_ascii_case("dark");
    let shell_on = cfg.shell.enabled;
    let root = if dark {
        format!(":root{{--bg:#0f172a;--card:#1e293b;--ink:#e2e8f0;--muted:#94a3b8;--line:#334155;--accent:{}}}", accent)
    } else {
        format!(":root{{--bg:#f4f6fb;--card:#ffffff;--ink:#1f2937;--muted:#6b7280;--line:#e5e7eb;--accent:{}}}", accent)
    };

    PAGE_TEMPLATE
        .replace("__ROOT__", &root)
        .replace("__ACCENT__", &accent)
        .replace("__TITLE__", &title)
        .replace("__SUBTITLE__", &subtitle)
        .replace("__SHELL_ON__", if shell_on { "true" } else { "false" })
}

fn esc_attr(s: &str) -> String {
    // 放进 JS 字符串 / HTML 属性里的最小转义。
    s.replace('\\', "\\\\").replace('"', "&quot;").replace('<', "&lt;").replace('\n', " ")
}

/// 终端页面占位：此处保留原 xterm.js 页面逻辑（复用 render_term）。
pub fn render_term(cfg: &crate::config::Config) -> String {
    let accent = esc_attr(&cfg.panel.accent);
    let title = esc_attr(&cfg.panel.title);
    let dark = cfg.panel.theme.eq_ignore_ascii_case("dark");
    let theme_css = if dark {
        "--bg:#0b1220;--line:#1e293b;"
    } else {
        "--bg:#ffffff;--line:#e5e7eb;"
    };
    TERM_TEMPLATE
        .replace("__TITLE__", &title)
        .replace("__ACCENT__", &accent)
        .replace("__CMD__", &esc_attr(&cfg.shell.cmd))
        .replace("__BG__", &theme_css)
}

// ---------------------------------------------------------------------------
// 管理控制台 HTML+JS（占位符注入，避免 format! 的双花括号问题）
// ---------------------------------------------------------------------------
const PAGE_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>__TITLE__ · Lumen</title>
<style>
__ROOT__
*{box-sizing:border-box;margin:0;padding:0}
html,body{height:100%}
body{font-family:system-ui,-apple-system,"PingFang SC","Microsoft YaHei",sans-serif;background:var(--bg);color:var(--ink);display:flex;flex-direction:column}
.layout{display:flex;flex:1;overflow:hidden}
.side{width:200px;padding:22px 14px;border-right:1px solid var(--line);flex:none;background:var(--card)}
.brand{font-weight:800;font-size:17px;margin:0 8px 20px;display:flex;align-items:center;gap:8px}
.brand i{width:30px;height:30px;border-radius:8px;background:var(--accent);color:#fff;display:inline-flex;align-items:center;justify-content:center;font-style:normal;font-weight:800}
.tab{display:block;width:100%;text-align:left;background:none;border:none;padding:11px 14px;border-radius:10px;font-size:14px;font-weight:600;color:var(--muted);cursor:pointer;margin-bottom:4px}
.tab:hover{background:#eef2f7}
.tab.on{background:var(--accent);color:#fff}
main{flex:1;overflow-y:auto;padding:28px 30px}
.hd{display:flex;align-items:baseline;justify-content:space-between;margin-bottom:18px}
.hd h1{font-size:22px;font-weight:800}
.hd .tag{color:var(--muted);font-size:12px}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:14px;margin-bottom:18px}
.card{background:var(--card);border:1px solid var(--line);border-radius:14px;padding:18px;box-shadow:0 6px 18px rgba(17,24,39,.05)}
.card .l{color:var(--muted);font-size:11px;letter-spacing:1px;text-transform:uppercase;margin-bottom:8px}
.card .v{font-size:26px;font-weight:800;font-variant-numeric:tabular-nums}
.card .s{color:var(--muted);font-size:12px;margin-top:5px}
.bar{height:8px;background:#eef2f7;border-radius:999px;overflow:hidden;margin-top:8px}
.bar i{display:block;height:100%;background:var(--accent);border-radius:999px;transition:width .5s}
.lbl{color:var(--muted);font-size:12px;margin:2px 8px}
svg{width:100%;height:120px;display:block}
table{width:100%;border-collapse:collapse;font-size:13px}
th,td{text-align:left;padding:9px 10px;border-bottom:1px solid var(--line)}
th{color:var(--muted);font-weight:600;font-size:12px}
td.num{font-variant-numeric:tabular-nums;text-align:right}
.hot{color:#dc2626;font-weight:700}.cool{color:#16a34a}.warm{color:#d97706}
button.mini{border:none;border-radius:7px;padding:5px 11px;font-size:12px;cursor:pointer;font-weight:600}
button.mini.danger{background:#fee2e2;color:#991b1b}
button.mini.act{background:#dbeafe;color:#1d4ed8}
button.mini.ok{background:#dcfce7;color:#166534}
button.pri{background:var(--accent);color:#fff;border:none;border-radius:10px;padding:10px 18px;font-weight:700;cursor:pointer}
input,select{background:var(--card);border:1px solid var(--line);border-radius:9px;padding:10px 12px;font-size:13px;color:inherit}
.toolbar{display:flex;gap:10px;margin-bottom:14px;flex-wrap:wrap}
form.rowform{display:flex;gap:10px;margin-bottom:14px;align-items:center;flex-wrap:wrap}
.toast{position:fixed;bottom:24px;right:24px;background:#111827;color:#fff;padding:11px 18px;border-radius:10px;font-size:13px;opacity:0;transition:.3s;z-index:50;max-width:80vw}
.toast.show{opacity:1}
.muted{color:var(--muted);font-size:12px}
@media(max-width:760px){.side{display:none}main{padding:16px}main:before{content:"☰  Lumen";font-weight:800;display:block;margin-bottom:14px}}
</style>
</head>
<body>
<div class="layout">
  <aside class="side">
    <div class="brand"><i>L</i>__TITLE__</div>
    <div id="tabs"></div>
  </aside>
  <main id="main">
    <div class="hd"><h1 id="ttl"></h1><div class="tag">__SUBTITLE__ · 实时面板</div></div>
    <div id="view"></div>
  </main>
</div>
<div class="toast" id="toast"></div>
<script>
var ACCENT='__ACCENT__', SHELL_ON=__SHELL_ON__;
var TITLES={ov:"系统概览",ps:"进程管理",sv:"服务管理",fw:"防火墙端口",tk:"定时任务"};
var TABS=[["ov","系统"],["ps","进程"],["sv","服务"],["fw","安全"],["tk","定时"]];
if(SHELL_ON)TABS.push(["term","终端"]);
var cur="ov";

function $(id){return document.getElementById(id)}
function esc(s){return (s||"").toString().replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;")}
function toast(m){var t=$('toast');t.textContent=m;t.className='toast show';setTimeout(function(){t.className='toast'},2200)}
function fmtB(b){var u=["B","KB","MB","GB","TB"],i=0;while(b>=1024&&i<u.length-1){b/=1024;i++}return(b.toFixed(1))+" "+u[i]}
function fmtUptime(s){var d=Math.floor(s/86400),h=Math.floor(s%86400/3600),m=Math.floor(s%3600/60);if(d)return d+"d "+h+"h";if(h)return h+"h "+m+"m";return m+"m"}

function renderTabs(){var el=$('tabs'),h="";for(var i=0;i<TABS.length;i++){h+='<button class="tab'+(TABS[i][0]===cur?' on':'')+'" data-t="'+TABS[i][0]+'">'+TABS[i][1]+'</button>'}el.innerHTML=h;
  el.querySelectorAll('.tab').forEach(function(b){b.onclick=function(){cur=b.dataset.t;renderTabs();choose()}})}
function choose(){window.clearInterval(window._iv);$('ttl').textContent=TITLES[cur]||"终端";
  if(cur==='term'){$('view').innerHTML='<div class="card" style="text-align:center;padding:40px"><a class="pri" style="text-decoration:none;display:inline-block" href="/term">&#9654; 打开 Web 终端</a></div>';return}
  if(cur==='ov'){loadOv();window._iv=setInterval(loadOv,2000)}
  if(cur==='ps')loadPs(); if(cur==='sv')loadSv(); if(cur==='fw')loadFw(); if(cur==='tk')loadTk();
}

function chart(cid){return {id:cid,path:"",draw:function(arr,color,max){var v=$('svg#c'+cid);if(!v)return;var w=v.clientWidth||300,h=v.clientHeight||120,n=arr.length;if(!n)return;var pad=2;function px(arr){var t="";for(var i=0;i<n;i++){var x=pad+(i/(n-1))*(w-2*pad);var val=Math.min(arr[i]||0,max||100);var y=h-pad-(h-2*pad)*(val/(max||100));t+=(i?"L":"M")+x.toFixed(1)+" "+y.toFixed(1)}return t}
      var t='<svg id="c'+cid+'" preserveAspectRatio="none"><path d="'+px(arr)+'" fill="none" stroke="'+color+'" stroke-width="2"/></svg>';
      v.innerHTML=t}}}

function loadOv(){
  fetch('/api/system').then(function(r){return r.json()}).then(function(d){
    var usedPct=(d.mem.total?d.mem.used/d.mem.total*100:0);
    var mupct=(d.mem.free>0?100:(0));
    var memBar=(100-mupct);
    memBar=(d.mem.total?d.mem.used/d.mem.total*100:0);
    var dd='<div class="grid">'
      +stat('内存',fmtB(d.mem.used)+' / '+fmtB(d.mem.total),'使用率 '+memBar.toFixed(1)+'%')
      +stat('CPU 负载',d.load.join(' · '),'/'+d.cores+' 核 · '+d.cpu.toFixed(1)+'%')
      +stat('运行时长',fmtUptime(d.uptime),d.host)
      +stat('网络速率','&#9660; '+fmtB(d.net.down)+'/s','&#9650; '+fmtB(d.net.up)+'/s')
      +'</div>';
    dd+='<div class="card" style="margin-bottom:14px"><div class="l">CPU 曲线(1m)</div><div id="ccpu">'+svgwrap('cpu')+'</div></div>';
    dd+='<div class="card" style="margin-bottom:14px"><div class="l">网络速率(1m)</div><div id="cnet">'+svgwrap('net')+'</div></div>';
    dd+='<div class="card"><div class="l">磁盘</div><table><tr><th>挂载点</th><th>设备</th><th class="num">容量</th><th class="num">已用</th><th class="num">使用</th></tr>';
    (d.disk||[]).forEach(function(x){dd+='<tr><td>'+esc(x.mount)+'</td><td class="muted">'+esc(x.fs)+'</td><td class="num">'+fmtB(x.total)+'</td><td class="num">'+fmtB(x.used)+'</td><td class="num">'+x.pct+'%</td></tr>'});
    dd+='</table></div>';
    $('view').innerHTML=dd;
    drawIn('cpu',d.series.cpu,'#2563eb',100);
    drawIn('net',d.series.net_up.map(function(v,i){return v}),'#16a34a',rateMax(d.series.net_dn,d.series.net_up));
    window._membar=memBar;
  });
}
function stat(l,v,s){return '<div class="card"><div class="l">'+l+'</div><div class="v">'+v+'</div><div class="s">'+s+'</div></div>'}
function svgwrap(id){return '<div id="host_'+id+'" style="height:120px"></div>'}
function rateMax(a,b){var m=1;(a||[]).forEach(function(x){m=Math.max(m,x||0)});(b||[]).forEach(function(x){m=Math.max(m,x||0)});return m||1}

function chartInst(id,host){var H=$('host_'+id);H.innerHTML='';H.innerHTML='<svg id="c'+id+'" preserveAspectRatio="none" style="width:100%;height:100%"></svg>';return $('c'+id)}
function drawIn(id,arr,color,max){if(!arr||!arr.length)return;var s=$('c'+id);if(!s)return;var parent=s.parentNode;var w=parent.clientWidth||300,h=parent.clientHeight||120;var t='<svg id="c'+id+'" preserveAspectRatio="none" style="width:100%;height:100%"><path d="'+sp(arr,max||100,w,h)+'" fill="none" stroke="'+color+'" stroke-width="2"/></svg>';parent.innerHTML=t;}
function sp(arr,max,w,h){var n=arr.length,pad=2,p="";for(var i=0;i<n;i++){var x=pad+(i/(n-1))*(w-2*pad);var val=Math.min(arr[i]||0,max);var y=h-pad-(h-2*pad)*(val/max);p+=(i?"L":"M")+x.toFixed(1)+" "+y.toFixed(1)}return p}

function loadPs(){fetch('/api/processes').then(function(r){return r.json()}).then(function(d){
  var h='<div class="toolbar"><input id="px" placeholder="搜索 PID / 名称" style="flex:1"><button class="pri" onclick="loadPs()">刷新</button></div><div class="card"><table><tr><th>PID</th><th>名称</th><th class="num">内存</th><th>状态</th><th></th></tr>';
  var kw=($('px')?$('px').value:'').toLowerCase();
  (d.list||[]).forEach(function(p){
    if(kw&&!(p.name.toLowerCase().indexOf(kw)>=0||String(p.pid)===kw))return;
    h+='<tr><td>'+p.pid+'</td><td>'+esc(p.name)+'</td><td class="num">'+fmtB(p.rss)+'</td><td>'+esc(p.state)+'</td>';
    h+='<td style="text-align:right"><button class="mini danger" onclick="killP('+p.pid+')">结束</button></td></tr>';
  });
  h+='</table>'+muted(d.list.length+' 个进程(按内存前 80)')+'</div>';
  $('view').innerHTML=h;
})}
function killP(pid){if(!confirm('确定结束进程 '+pid+' ？'))return;post('/api/process/kill',{pid:pid},function(){toast('已请求结束 '+pid);loadPs()})}

function loadSv(){fetch('/api/services').then(function(r){return r.json()}).then(function(d){
  var h='<div class="toolbar"><input id="sx" placeholder="搜索服务名" style="flex:1"><button class="pri" onclick="loadSv()">刷新</button></div><div class="card"><table><tr><th>服务</th><th>状态</th><th>说明</th><th></th></tr>';
  var kw=($('sx')?$('sx').value:'').toLowerCase();
  (d.list||[]).forEach(function(s){
    if(kw&&s.name.toLowerCase().indexOf(kw)<0)return;
    var st=s.active==='active'?'<span class="cool">● running</span>':'<span class="'+(s.active==='failed'?'hot':'warm')+'">● '+esc(s.active)+'</span>';
    h+='<tr><td>'+esc(s.name)+'</td><td>'+st+'</td><td class="muted">'+esc(s.desc)+'</td>';
    h+='<td style="text-align:right">'+miniAct('启动','ok',s.name,'start')+miniAct('重启','act',s.name,'restart')+miniAct('停止','danger',s.name,'stop')+'</td></tr>';
  });
  h+='</table>'+muted(d.list.length+' 个服务')+'</div>';
  $('view').innerHTML=h;
})}
function miniAct(t,c,n,a){return '<button class="mini '+c+'" onclick="svAct(\''+n+'\',\''+a+'\')">'+t+'</button> '}
function svAct(n,a){post('/api/service/action',{name:n,action:a},function(res){toast(res.msg||('已'+a+' '+n));loadSv()})}

function loadFw(){fetch('/api/firewall').then(function(r){return r.json()}).then(function(d){
  var h='<form class="rowform" onsubmit="fwAdd(event)"><input name="port" placeholder="端口 或 端口/协议(tcp/udp)，如 8080" required style="min-width:260px;flex:1"><button class="pri" type="submit">+ 放行端口</button></form>';
  h+='<div class="card"><table><tr><th>端口/协议</th><th>动作</th><th></th></tr>';
  (d.list||[]).forEach(function(f){h+='<tr><td>'+esc(f.port)+'</td><td><span class="cool">'+esc(f.action)+'</span></td><td style="text-align:right"><button class="mini danger" onclick="fwDel(\''+esc(f.port)+'\')">删除</button></td></tr>'});
  h+='</table>'+muted((d.list||[]).length+' 条放行规则 (ufw)')+'</div>';
  $('view').innerHTML=h;
})}
function fwAdd(e){e.preventDefault();var p=e.target.port.value.trim();if(!p)return;post('/api/firewall/add',{port:p},function(res){toast(res.msg||'已放行 '+p);loadFw()})}
function fwDel(p){if(!confirm('删除放行规则 '+p+' ?'))return;post('/api/firewall/del',{port:p},function(res){toast(res.msg||'已删除');loadFw()})}

function loadTk(){fetch('/api/tasks').then(function(r){return r.json()}).then(function(d){
  var h='<form class="rowform" onsubmit="tkAdd(event)"><input name="schedule" placeholder="cron 5 段，如 0 2 * * *" required style="width:220px"><input name="command" placeholder="执行命令，如 bash /opt/backup.sh" required style="flex:1;min-width:220px"><button class="pri" type="submit">+ 添加任务</button></form>';
  h+='<div class="card"><table><tr><th>调度</th><th>命令</th></tr>';
  (d.list||[]).forEach(function(t){h+='<tr><td><code>'+esc(t.schedule)+'</code></td><td>'+esc(t.command)+'</td></tr>'});
  h+='</table>'+muted((d.list||[]).length+' 条定时任务 (crontab)')+'</div>';
  $('view').innerHTML=h;
})}
function tkAdd(e){e.preventDefault();var sch=e.target.schedule.value.trim(),cmd=e.target.command.value.trim();if(!sch||!cmd)return;post('/api/tasks/add',{schedule:sch,command:cmd},function(res){toast(res.msg||'已添加');loadTk()})}

function post(path,fields,cb){var fd=new FormData();for(var k in fields)fd.append(k,fields[k]);fetch(path,{method:'POST',body:fd}).then(function(r){return r.json()}).then(function(res){if(cb)cb(res);if(res&&res.ok===false&&!cb)toast(res.msg||'操作失败')})}
function muted(s){return '<div class="muted" style="padding:8px 2px 0">'+s+'</div>'}

renderTabs();choose();
</script>
</body>
</html>
"#;

const TERM_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>终端 · __TITLE__</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.5.0/css/xterm.min.css">
<style>
*{box-sizing:border-box}
body{margin:0;padding:0;background:__BG__;font-family:system-ui,sans-serif;height:100vh;display:flex;flex-direction:column}
.top{display:flex;align-items:center;gap:12px;padding:14px 18px;border-bottom:1px solid var(--line,transparent)}
.top .mk{width:44px;height:44px;border-radius:12px;background:__ACCENT__;color:#fff;display:flex;align-items:center;justify-content:center;font-weight:800;font-size:20px}
.top h1{font-size:15px;margin:0;font-weight:800}
.top .hint{font-size:11px;color:#6b7280}
.back{margin-left:auto;color:__ACCENT__;text-decoration:none;font-size:13px;font-weight:700}
#term{flex:1;padding:6px 0}
.status{height:26px;font-size:11px;color:#6b7280;display:flex;align-items:center;padding:0 18px;border-top:1px solid var(--line,transparent)}
</style>
</head>
<body>
<div class="top"><div class="mk">&#9654;</div><div><h1>__TITLE__ · Web 终端</h1><div class="hint">__CMD__</div></div><a class="back" href="/">&#8592; 返回面板</a></div>
<div id="term"></div>
<div class="status"><span id="st">连接中…</span></div>
<script src="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.5.0/lib/xterm.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0.10.0/lib/addon-fit.min.js"></script>
<script>
(function(){
  var term=new Terminal({cursorBlink:true,fontSize:14});
  var fit=new FitAddon.FitAddon();term.loadAddon(fit);term.open(document.getElementById('term'));fit.fit();
  var ws=new WebSocket((location.protocol==='https:'?'wss://':'ws://')+location.host+'/ws');
  var st=document.getElementById('st');
  ws.onopen=function(){st.textContent='已连接 · '+term.cols+'x'+term.rows;term.focus()};
  ws.onclose=function(){st.textContent='已断开';term.dispose()};
  ws.onmessage=function(ev){term.write(ev.data)};
  var enc=new TextEncoder();
  term.onData(function(d){ws.send(enc.encode(d))});
  function sendSize(){ws.send('st\t'+term.cols+'\t'+term.rows)}
  term.onResize(sendSize);sendSize();
  window.addEventListener('resize',function(){fit.fit();sendSize()});
})();
</script>
</body>
</html>
"#;