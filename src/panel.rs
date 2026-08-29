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

/// 登录 / 初始设置页。开启认证且未设置密码时走初始设置向导。
pub fn render_login(state: &State) -> String {
    let cfg = &state.cfg;
    let accent = esc_attr(&cfg.panel.accent);
    let title = esc_attr(&cfg.panel.title);
    let dark = cfg.panel.theme.eq_ignore_ascii_case("dark");
    let card = if dark { "background:#1e293b;border:1px solid #334155" } else { "background:#fff;border:1px solid #e5e7eb" };
    let ink = if dark { "#e2e8f0" } else { "#1f2937" };
    let sub = if dark { "#94a3b8" } else { "#6b7280" };
    let bg = if dark { "#0f172a" } else { "#f4f6fb" };
    LOGIN_TEMPLATE
        .replace("__ACCENT__", &accent)
        .replace("__TITLE__", &title)
        .replace("__CARD__", card)
        .replace("__INK__", ink)
        .replace("__SUB__", sub)
        .replace("__BG__", bg)
}

/// 登录/初始设置页面（纯内联，无外部资源）。
const LOGIN_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>登录 · __TITLE__</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{font-family:system-ui,-apple-system,"PingFang SC","Microsoft YaHei",sans-serif;background:__BG__;display:flex;align-items:center;justify-content:center;min-height:100vh;color:__INK__}
.card{width:360px;max-width:92vw;padding:34px 30px;border-radius:16px;__CARD__;box-shadow:0 10px 30px rgba(0,0,0,.06)}
.logo{width:46px;height:46px;border-radius:12px;background:__ACCENT__;color:#fff;display:flex;align-items:center;justify-content:center;font-size:22px;font-weight:800;margin:0 auto 16px}
h1{font-size:19px;font-weight:800;text-align:center;margin-bottom:6px}
p.sub{font-size:13px;color:__SUB__;text-align:center;margin-bottom:24px}
label{display:block;font-size:13px;color:__SUB__;margin:14px 0 6px}
input{width:100%;padding:11px 12px;border:1px solid #d1d5db;border-radius:9px;font-size:14px;background:transparent;color:__INK__}
input:focus{outline:none;border-color:__ACCENT__;box-shadow:0 0 0 3px rgba(37,99,235,.12)}
.row{display:flex;align-items:center;justify-content:space-between;margin:16px 0 6px;font-size:13px;color:__SUB__}
.row label{margin:0;cursor:pointer}
button{width:100%;padding:12px;border:none;border-radius:9px;background:__ACCENT__;color:#fff;font-size:15px;font-weight:700;cursor:pointer;margin-top:18px}
button:disabled{opacity:.6;cursor:not-allowed}
.msg{text-align:center;font-size:13px;margin-top:14px;min-height:18px;color:#dc2626}
.loading{text-align:center;color:__SUB__;font-size:14px;padding:40px 0}
</style>
</head>
<body>
<div class="card" id="card" hidden>
  <div class="logo">V</div>
  <h1 id="head">欢迎使用 __TITLE__</h1>
  <p class="sub" id="sub">登录面板</p>
  <form id="form">
    <label for="p1" id="lab1">密码</label>
    <input type="password" id="p1" autocomplete="current-password" placeholder="输入管理员密码">
    <label for="p2" id="lab2" hidden>确认密码</label>
    <input type="password" id="p2" autocomplete="new-password" placeholder="再次输入" hidden>
    <div class="row"><label><input type="checkbox" id="rem"> 记住我（30 天）</label></div>
    <button id="btn" type="submit">登 录</button>
  </form>
  <div class="msg" id="msg"></div>
</div>
<div class="loading" id="load">加载中…</div>
<script>
let setup=false;
async function init(){
  try{
    const r=await fetch('/api/auth/state',{credentials:'same-origin'});
    const d=await r.json();
    setup=!!d.needs_setup;
  }catch(e){}
  document.getElementById('load').hidden=true;
  document.getElementById('card').hidden=false;
  if(setup){
    document.getElementById('head').textContent='初始化面板';
    document.getElementById('sub').textContent='设置管理员密码';
    document.getElementById('lab1').textContent='管理员密码';
    document.getElementById('lab2').hidden=false;
    document.getElementById('p2').hidden=false;
    document.getElementById('btn').textContent='创建并登录';
  }
}
document.getElementById('form').addEventListener('submit',async function(ev){
  ev.preventDefault();
  const p1=document.getElementById('p1').value;
  const p2=document.getElementById('p2').value;
  const rem=document.getElementById('rem').checked;
  const msg=document.getElementById('msg');
  msg.textContent='';
  if(p1.length<4){msg.textContent='密码至少 4 位';return;}
  if(setup && p1!==p2){msg.textContent='两次输入不一致';return;}
  const url=setup?'/api/setup':'/api/login';
  const btn=document.getElementById('btn');
  btn.disabled=true;
  try{
    const r=await fetch(url,{method:'POST',credentials:'same-origin',
      headers:{'Content-Type':'application/json'},
      body:JSON.stringify({password:p1,remember:rem})});
    const d=await r.json();
    if(d.ok){location.href='/';}
    else{msg.textContent=d.msg||'登录失败';}
  }catch(e){msg.textContent='请求失败';}
  btn.disabled=false;
});
init();
</script>
</body>
</html>
"#;

// ---------------------------------------------------------------------------
// 管理控制台 HTML+JS（占位符注入，避免 format! 的双花括号问题）
// ---------------------------------------------------------------------------
const PAGE_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>__TITLE__ · vPanel</title>
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
@media(max-width:760px){.side{display:none}main{padding:16px}main:before{content:"☰  vPanel";font-weight:800;display:block;margin-bottom:14px}}
</style>
</head>
<body>
<div class="layout">
  <aside class="side">
    <div class="brand"><i>v</i>__TITLE__</div>
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
var TITLES={ov:"系统概览",ps:"进程管理",sv:"服务管理",fw:"防火墙端口",al:"告警通知",tk:"定时任务",shop:"软件商店",mcp:"AI 工具",plg:"插件",inf:"系统信息",net:"网络连接",log:"实时日志",fs:"文件管理",dk:"磁盘占用",rp:"反向代理",web:"网站管理",db:"数据库",env:"环境",cert:"SSL证书",bk:"备份",hd:"安全加固",rs:"资源排行"};
var TABS=[["ov","系统"],["ps","进程"],["sv","服务"],["web","网站"],["db","数据库"],["env","环境"],["cert","证书"],["bk","备份"],["hd","安全"],["fw","防火墙"],["al","告警"],["tk","定时"],["rp","反代"],["shop","商店"],["mcp","AI"],["plg","插件"],["inf","信息"],["net","连接"],["log","日志"],["fs","文件"],["dk","磁盘"],["rs","资源"]];
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
  if(cur==='ps')loadPs(); if(cur==='sv')loadSv(); if(cur==='fw')loadFw(); if(cur==='al')loadAl(); if(cur==='tk')loadTk();
  if(cur==='shop')loadShop(); if(cur==='mcp')loadMCP(); if(cur==='plg')loadPlug();
  if(cur==='inf')loadInf(); if(cur==='net'){loadNet();window._iv=setInterval(loadNet,3000)}
  if(cur==='log')loadLogCfg(); if(cur==='fs')loadFs('/');
  if(cur==='dk')loadDk('/'); if(cur==='rp')loadRp(); if(cur==='web')loadWeb();
  if(cur==='db')loadDb(); if(cur==='env')loadEnv(); if(cur==='cert')loadCert(); if(cur==='bk')loadBk(); if(cur==='hd')loadHd();
  if(cur==='rs'){loadRs();window._iv=setInterval(loadRs,5000)}
}
function getJson(path,cb){fetch(path).then(function(r){return r.json()}).then(function(j){cb(j||{})}).catch(function(){cb({ok:false})})}
function act(path,fields){post(path,fields,function(res){toast(res.msg||(res.ok?'操作成功':'操作失败'));setTimeout(reload,800)})}
function fmtSize(n){if(n==null)return '-';if(n>1073741824)return(n/1073741824).toFixed(2)+'G';if(n>1048576)return(n/1048576).toFixed(1)+'M';if(n>1024)return(n/1024).toFixed(0)+'K';return n+'B'}
function fmtTime(t){if(!t)return '-';var d=new Date((t<1e12?t*1000:t));return d.getFullYear()+'-'+('0'+(d.getMonth()+1)).slice(-2)+'-'+('0'+d.getDate()).slice(-2)+' '+('0'+d.getHours()).slice(-2)+':'+('0'+d.getMinutes()).slice(-2)}
function loadDb(){
  var b=$('#box');
  b.innerHTML='<div class="toolbar"><span class="l" style="font-weight:700">数据库</span><button class="mini pri" onclick="loadDb()">刷新</button></div>'+
    '<div class="card" id="dbstc"><span class="muted">加载中…</span></div>'+
    '<div class="gridg" style="grid-template-columns:1fr 1fr;gap:12px">'+
      '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">数据库操作</div>'+
        '<div class="row"><input id="dbname" placeholder="库名" style="flex:1"><input id="dbcs" placeholder="字符集(utf8mb4)" style="width:130px"><button class="mini ok" onclick="dbCreate()">建库</button></div>'+
        '<div class="row"><input id="dbname2" placeholder="删除的库名" style="flex:1"><button class="mini danger" onclick="dbDrop()">删库</button></div>'+
        '<div class="row"><input id="dbuser" placeholder="用户名" style="flex:1"><input id="dbpass" type="password" placeholder="密码" style="flex:1"><input id="dbhost" placeholder="host(localhost)" style="width:100px"><button class="mini ok" onclick="dbUserCreate()">建用户</button></div>'+
        '<div class="row"><input id="grantdb" placeholder="授权库" style="flex:1"><input id="grantuser" placeholder="用户" style="flex:1"><button class="mini" onclick="dbGrant()">授权</button></div>'+
        '<div class="row"><input id="backupdb" placeholder="备份的库名" style="flex:1"><button class="mini" onclick="dbBackup()">备份</button></div>'+
        '<div class="row"><input id="newroot" type="password" placeholder="新root密码" style="flex:1"><button class="mini danger" onclick="dbResetRoot()">重置root密码</button></div>'+
      '</div>'+
      '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">用户列表</div><div id="dbusers" class="muted">加载中…</div></div>'+
      '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">数据库列表</div><div id="dbnames" class="muted">加载中…</div></div>'+
      '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">备份文件</div><div id="dbbackups" class="muted">加载中…</div></div>'+
    '</div>';
  getJson('/api/db/status',function(j){var s=j.installed?(j.running?'<span class="cool">运行中</span>':'<span class="warm">已装但未运行</span>'):'<span class="hot">未安装</span>';$('#dbstc').innerHTML='状态：<b>'+s+'</b> · 用户：<code>'+esc(j.user||'-')+'</code>'});
  getJson('/api/db/users',function(j){var d=(j&&j.data)||[];$('#dbusers').innerHTML=d.length?'<div vp-tbl>'+d.map(function(u){return'<div class="kr"><b>'+esc(u.user||'?')+'</b><span class="muted">@'+esc(u.host||'%')+'</span></div>'}).join('')+'</div>':'<span class="muted">暂无用户</span>'});
  getJson('/api/db/databases',function(j){var d=(j&&j.data)||[];$('#dbnames').innerHTML=d.length?'<div vp-tbl>'+d.map(function(n){return'<div class="kr"><b>'+esc(n)+'</b><span><button class="mini" onclick="dbBackupName(\''+n+'\')">备份</button><button class="mini danger" onclick="dbDropName(\''+n+'\')">删除</button></span></div>'}).join('')+'</div>':'<span class="muted">暂无数据库</span>'});
  getJson('/api/db/backups',function(j){var d=(j&&j.list)||[];$('#dbbackups').innerHTML=d.length?'<div vp-tbl>'+d.map(function(x){return'<div class="kr"><b>'+esc(x.name)+'</b><span class="muted">'+fmtSize(x.size)+'</span><span class="muted">'+fmtTime(x.time)+'</span><button class="mini" onclick="dbRestore(\''+x.name+'\')">恢复</button></div>'}).join('')+'</div>':'<span class="muted">暂无备份</span>'});
}
function dbCreate(){var n=$('#dbname').value.trim();if(!n)return toast('请输入库名');act('/api/db/create_db',{db:n,charset:$('#dbcs').value||'utf8mb4'})}
function dbDrop(){var n=$('#dbname2').value.trim();if(!n)return toast('请输入库名');if(!confirm('确认删除数据库 '+n+' ？'))return;act('/api/db/drop_db',{db:n})}
function dbUserCreate(){act('/api/db/create_user',{user:$('#dbuser').value,pass:$('#dbpass').value,host:$('#dbhost').value||'localhost'})}
function dbGrant(){act('/api/db/grant',{db:$('#grantdb').value,user:$('#grantuser').value,host:$('#dbhost').value||'localhost'})}
function dbBackup(){var n=$('#backupdb').value.trim();if(!n)return toast('请输入库名');act('/api/db/backup',{db:n})}
function dbResetRoot(){var p=$('#newroot').value;if(!p||p.length<4)return toast('密码至少4位');if(!confirm('确认重置数据库 root 密码？'))return;act('/api/db/reset_root',{password:p})}
function dbBackupName(n){act('/api/db/backup',{db:n})}
function dbDropName(n){if(!confirm('确认删除数据库 '+n+' ？'))return;act('/api/db/drop_db',{db:n})}
function dbRestore(n){if(!confirm('确认从 '+n+' 恢复？'))return;act('/api/db/restore',{file:n})}

function loadEnv(){
  var b=$('#box');
  b.innerHTML='<div class="toolbar"><span class="l" style="font-weight:700">环境运行时</span><span class="muted" style="align-self:center">安装需系统软件源可达</span><button class="mini pri" onclick="loadEnv()">刷新</button></div>'+
    '<div id="envlist" class="muted" style="padding:12px 0">加载中…</div>';
  getJson('/api/env',function(j){var l=(j&&j.list)||[];if(!l.length){$('#envlist').innerHTML='<span class="muted">暂无环境信息</span>';return}
    var h='';l.forEach(function(r){
      var st=r.installed?(r.running?'<span class="cool">运行中</span>':'<span class="warm">已停</span>'):'<span class="hot">未安装</span>';
      var btns=r.installed
        ?'<button class="mini ok" onclick="envAct(\''+r.id+'\',\'start\')">启动</button><button class="mini" onclick="envAct(\''+r.id+'\',\'restart\')">重启</button><button class="mini danger" onclick="envAct(\''+r.id+'\',\'stop\')">停止</button>'
        :'<button class="mini ok" onclick="envInstall(\''+r.id+'\')">安装</button>';
      h+='<div class="kr"><div><b>'+esc(r.name)+'</b> <span class="muted">v'+esc(r.version||'-')+'</span></div>'+st+'<span>'+btns+'</span></div>';
    });
    $('#envlist').innerHTML='<div vp-tbl>'+h+'</div><div class="muted" style="margin-top:8px">备注：'+l.map(function(r){return esc(r.remark)}).join(' · ')+'</div>';
  })
}
function envAct(id,action){var btn=event&&event.target;if(btn){btn.disabled=true;btn.textContent='…'};act('/api/env/service',{id:id,action:action})}
function envInstall(id){var btn=event&&event.target;if(btn){btn.disabled=true;btn.textContent='安装中…'};act('/api/env/install',{id:id})}

function loadCert(){
  var b=$('#box');
  b.innerHTML='<div class="toolbar"><span class="l" style="font-weight:700">SSL 证书</span><span class="muted" style="align-self:center">目录 <span id="certsdir">-</span></span><button class="mini pri" onclick="loadCert()">刷新</button></div>'+
    '<div class="gridg" style="grid-template-columns:1fr 1fr;gap:12px">'+
      '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">签发证书</div>'+
        '<div class="row"><input id="cname" placeholder="证书名" style="flex:1"><input id="cdomain" placeholder="域名" style="flex:1"><button class="mini ok" onclick="certSelfSigned()">自签</button></div>'+
        '<div class="row" style="margin-top:8px"><input id="cdomain2" placeholder="域名" style="flex:1"><input id="cwebroot" placeholder="webroot目录" style="flex:1"><button class="mini" onclick="certLeIssue()">Let\'s Encrypt</button></div>'+
      '</div>'+
      '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">导入已有证书</div>'+
        '<div class="row"><input id="cimportname" placeholder="证书名" style="flex:1"><button class="mini" onclick="certImport()">导入</button></div>'+
        '<textarea id="cimpfc" rows="2" placeholder="fullchain.pem 内容" style="width:100%;margin-top:6px;box-sizing:border-box"></textarea>'+
        '<textarea id="cimpk" rows="2" placeholder="privkey.key 内容" style="width:100%;margin-top:4px;box-sizing:border-box"></textarea>'+
      '</div>'+
    '</div>'+
    '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">已签发证书</div><div id="certlist" class="muted">加载中…</div></div>';
  getJson('/api/ssl',function(j){$('#certsdir').textContent=j.dir||'-';var l=(j&&j.list)||[];if(!l.length){$('#certlist').innerHTML='<span class="muted">暂无证书</span>';return}
    $('#certlist').innerHTML='<div vp-tbl>'+l.map(function(c){
      return'<div class="kr"><div><b>'+esc(c.name)+'</b> <span class="muted">'+esc(c.domain||'-')+'</span></div><span class="muted">到期：'+esc(c.not_after||'-')+'</span><span>'+(c.ok?'<span class="cool">有效</span>':'<span class="hot">无效</span>')+'</span><button class="mini" onclick="certApply(\''+c.name+'\')">套用到站点</button></div>'
    }).join('')+'</div>'
  })
}
function certSelfSigned(){var n=$('#cname').value.trim(),d=$('#cdomain').value.trim();if(!n||!d)return toast('请填写证书名和域名');act('/api/ssl/self_signed',{name:n,domain:d,days:365})}
function certLeIssue(){var d=$('#cdomain2').value.trim();if(!d)return toast('请填写域名');act('/api/ssl/le_issue',{name:d.replace(/\./g,'_'),domain:d,webroot:$('#cwebroot').value||'/var/www/html'})}
function certImport(){var n=$('#cimportname').value.trim();if(!n)return toast('请填写证书名');act('/api/ssl/import',{name:n,fullchain:$('#cimpfc').value,privkey:$('#cimpk').value})}
function certApply(n){var site=prompt('套用证书到哪个站点名？');if(!site)return;act('/api/ssl/apply',{site:site,cert:n,upgrade:true})}

function loadBk(){
  var b=$('#box');
  b.innerHTML='<div class="toolbar"><span class="l" style="font-weight:700">备份</span><button class="mini pri" onclick="loadBk()">刷新</button></div>'+
    '<div class="gridg" style="grid-template-columns:1fr 1fr;gap:12px">'+
      '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">手动备份</div>'+
        '<div class="row"><button class="mini ok" onclick="bkRun()">立即全量备份</button></div>'+
        '<div class="row" style="margin-top:8px"><input id="bksrc" placeholder="要备份的目录路径" style="flex:1"><input id="bkkeep" placeholder="保留份数" value="5" style="width:80px"><button class="mini" onclick="bkDir()">目录备份</button></div>'+
      '</div>'+
      '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">定时备份</div>'+
        '<div class="row"><input id="bkcron" placeholder="cron 表达式, 默认每天2:00" style="flex:1"><button class="mini" onclick="bkSched()">设置</button></div>'+
        '<div class="row" style="margin-top:8px"><button class="mini danger" onclick="bkSchedRemove()">移除定时备份</button></div>'+
      '</div>'+
    '</div>'+
    '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">备份配置 <span id="bkcfg" class="muted" style="font-weight:400">加载中…</span></div></div>'+
    '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">备份文件</div><div id="bkflist" class="muted">加载中…</div></div>';
  getJson('/api/backup',function(j){$('#bkcfg').textContent='目录：'+(j.dir||'-')+'，保留 '+esc(j.keep)+' 份';var f=(j.files||[]);if(!f.length){$('#bkflist').innerHTML='<span class="muted">暂无备份文件</span>';return}
    $('#bkflist').innerHTML='<div vp-tbl>'+f.map(function(x){return'<div class="kr"><b>'+esc(x.name)+'</b><span class="muted">'+fmtSize(x.size)+'</span><span class="muted">'+fmtTime(x.mtime)+'</span><button class="mini" onclick="bkCloud(\''+x.name.replace(/'/g,"")+'\')">上传云</button></div>'}).join('')+'</div>'
  })
}
function bkRun(){act('/api/backup/run',{})}
function bkDir(){act('/api/backup/dir',{path:$('#bksrc').value,keep:$('#bkkeep').value||5})}
function bkSched(){act('/api/backup/schedule',{cron:$('#bkcron').value||'0 2 * * *'})}
function bkSchedRemove(){if(!confirm('确认移除定时备份？'))return;act('/api/backup/schedule_remove',{})}
function bkCloud(f){if(!confirm('上传 '+f+' 到云存储？'))return;act('/api/backup/cloud',{file:f})}

function loadHd(){
  var b=$('#box');
  b.innerHTML='<div class="toolbar"><span class="l" style="font-weight:700">安全加固</span><button class="mini pri" onclick="loadHd()">刷新</button></div>'+
    '<div class="gridg" style="grid-template-columns:1fr 1fr;gap:12px">'+
      '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">SSH 加固</div>'+
        '<div class="row"><span id="hdstate" class="muted">加载中…</span></div>'+
        '<div class="row" style="margin-top:8px"><button class="mini" onclick="harden(true)">禁止root密码登录</button><button class="mini danger" onclick="unharden()">撤销加固</button></div>'+
        '<div class="muted" style="margin-top:6px;font-size:12px">加固将写入 sshd_config.d，先经 sshd -t 校验，失败自动回滚。</div>'+
      '</div>'+
      '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">WAF 防护</div>'+
        '<div class="row"><span id="wafstate" class="muted">加载中…</span></div>'+
        '<div class="row" style="margin-top:8px"><input id="wafrps" placeholder="每秒请求上限" value="30" style="width:110px"><input id="wafburst" placeholder="突发上限" value="50" style="width:90px"><button class="mini ok" onclick="wafOn()">开启</button><button class="mini danger" onclick="wafOff()">关闭</button></div>'+
      '</div>'+
    '</div>'+
    '<div class="card"><div class="l" style="font-weight:600;margin-bottom:10px">IP 封禁管理</div>'+
      '<div class="row"><input id="banip" placeholder="要封禁的 IP" style="flex:1"><button class="mini" onclick="banIP()">封禁</button><button class="mini" onclick="bruteScan()">扫描并封禁暴力破解</button></div>'+
      '<div id="banlist" class="muted" style="margin-top:8px">加载中…</div>'+
    '</div>';
  getJson('/api/security/hardening',function(j){$('#hdstate').innerHTML=(j.on)?'<span class="cool">已加固</span>':'<span class="warm">未加固</span>'});
  getJson('/api/security/waf',function(j){$('#wafstate').innerHTML=(j.on)?'<span class="cool">已开启</span>':'<span class="warm">已关闭</span>'});
  getJson('/api/security/bans',function(j){var l=(j.list)||[];if(!l.length){$('#banlist').innerHTML='<span class="muted">暂无封禁 IP</span>';return}
    $('#banlist').innerHTML='<div vp-tbl>'+l.map(function(ip){return'<div class="kr"><b>'+esc(ip)+'</b><button class="mini" onclick="unbanIP(\''+ip+'\')">解封</button></div>'}).join('')+'</div>'
  })
}
function harden(nr){if(!confirm('确认 SSH 加固？'))return;act('/api/security/harden',{no_root_pass:(nr?'1':'0'),no_password:'1'})}
function unharden(){if(!confirm('确认撤销 SSH 加固？'))return;act('/api/security/unharden',{})}
function wafOn(){if(!confirm('开启 WAF？'))return;act('/api/security/waf/enable',{rps:$('#wafrps').value||30,burst:$('#wafburst').value||50})}
function wafOff(){if(!confirm('关闭 WAF？'))return;act('/api/security/waf/disable',{})}
function banIP(){var ip=$('#banip').value.trim();if(!ip)return toast('请输入IP');act('/api/security/ban',{ip:ip})}
function unbanIP(ip){act('/api/security/unban',{ip:ip})}
function bruteScan(){if(!confirm('扫描认证日志并封禁高失败IP？'))return;act('/api/security/brute',{threshold:5})}

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
  var on=d.enabled;
  var h='<div class="toolbar"><span class="muted" style="align-self:center">后端: <code>'+esc(d.backend||'none')+'</code> · 状态: <b>'+(on?'启用':'停用')+'</b></span>';
  h+=(on?'<button class="mini danger" onclick="fwEnable(false)">停用防火墙</button>':'<button class="mini ok" onclick="fwEnable(true)">启用防火墙</button>')+'</div>';
  h+='<form class="rowform" onsubmit="fwAdd(event)"><select name="action"><option value="allow">放行</option><option value="deny">拒绝</option></select>';
  h+='<select name="proto"><option value="tcp">TCP</option><option value="udp">UDP</option><option value="both">TCP/UDP</option></select>';
  h+='<input name="port" placeholder="端口(留空=全部)，如 8080 或 8080-9090" style="flex:1;min-width:130px">';
  h+='<input name="ip" placeholder="来源 IP/网段(留空=任意)，如 1.2.3.0/24" style="flex:1;min-width:150px">';
  h+='<button class="pri" type="submit">+ 添加规则</button></form>';
  h+='<div class="card"><table><tr><th>#</th><th>动作</th><th>端口</th><th>协议</th><th>来源</th><th></th></tr>';
  (d.list||[]).forEach(function(f){
    var act=f.action==='deny'?'<span style="color:#dc2626;font-weight:600">拒绝</span>':'<span style="color:#16a34a;font-weight:600">放行</span>';
    h+='<tr><td>'+f.id+'</td><td>'+act+'</td><td><code>'+esc(f.port||'全部')+'</code></td><td>'+esc(f.proto||'tcp')+'</td><td>'+esc(f.ip||'任意')+'</td><td style="text-align:right"><button class="mini danger" onclick="fwDel('+f.id+')">删除</button></td></tr>';
  });
  h+='</table>'+muted((d.list||[]).length+' 条规则 · 自研独立链(nft/iptables)，不依赖 ufw')+'</div>';
  $('view').innerHTML=h;
})}
function fwAdd(e){e.preventDefault();post('/api/firewall/add',{action:e.target.action.value,proto:e.target.proto.value,port:e.target.port.value,ip:e.target.ip.value},function(res){toast(res.msg||'已添加');loadFw()})}
function fwDel(id){if(!confirm('删除规则 #'+id+' ?'))return;post('/api/firewall/del',{id:id},function(res){toast(res.msg||'已删除');loadFw()})}
function fwEnable(on){post(on?'/api/firewall/enable':'/api/firewall/disable',{},function(res){toast(res.msg||(on?'已启用':'已停用'));loadFw()})}

function loadAl(){fetch('/api/alert').then(function(r){return r.json()}).then(function(d){
  var on=d.enabled;
  var h='<div class="toolbar"><span class="muted" style="align-self:center">SMTP 告警 · 加密'+(d.tls?'已支持':'未支持(需 --features tls)')+'</span>';
  h+=(on?'<button class="mini danger" onclick="alEnable(false)">停用告警</button>':'<button class="mini ok" onclick="alEnable(true)">启用告警</button>')+' <button class="mini" onclick="alTest()">发送测试邮件</button></div>';
  h+='<form class="gridg" style="grid-template-columns:1fr 1fr;gap:12px" onsubmit="alSave(event)">';
  h+='<div class="card"><b>SMTP 设置</b>'
    +'<div class="row"><input id="alhost" placeholder="SMTP 服务器，如 smtp.qq.com" value="'+esc(d.smtp_host)+'" style="flex:1"><input id="alport" placeholder="端口" value="'+(d.smtp_port||587)+'" style="width:76px"></div>'
    +'<div class="row"><input id="aluser" placeholder="账号（可空）" value="'+esc(d.smtp_user)+'" style="flex:1"><input id="alpass" type="password" placeholder="密码（留空不改）" style="flex:1"></div>'
    +'<div class="row"><select id="almode"><option value="starttls"'+(d.mode==='starttls'?' selected':'')+'>STARTTLS (587)</option><option value="ssl"'+(d.mode==='ssl'?' selected':'')+'>SSL/TLS (465)</option><option value="none"'+(d.mode==='none'?' selected':'')+'>明文 (25)</option></select><span class="muted">加密方式</span></div>'
    +'<div class="row"><input id="alfrom" placeholder="发件人邮箱" value="'+esc(d.from)+'" style="flex:1"></div>'
    +'<div class="row"><input id="alto" placeholder="收件人邮箱（可逗号分隔多个）" value="'+esc(d.to)+'" style="flex:1"></div>'
    +'</div>';
  h+='<div class="card"><b>告警阈值（0 = 关闭该项）</b>'
    +'<div class="row"><span class="muted" style="width:120px">CPU &ge;</span><input id="alcpu" type="number" step="any" min="0" value="'+(d.cpu||0)+'" style="width:84px"><span class="muted">%&nbsp;&nbsp;当前 '+d.current.cpu.toFixed(1)+'%</span></div>'
    +'<div class="row"><span class="muted" style="width:120px">内存 &ge;</span><input id="almem" type="number" step="any" min="0" value="'+(d.mem||0)+'" style="width:84px"><span class="muted">%&nbsp;&nbsp;当前 '+d.current.mem+'%</span></div>'
    +'<div class="row"><span class="muted" style="width:120px">磁盘(/) &ge;</span><input id="aldisk" type="number" step="any" min="0" value="'+(d.disk||0)+'" style="width:84px"><span class="muted">%&nbsp;&nbsp;当前 '+d.current.disk+'%</span></div>'
    +'<div class="row"><span class="muted" style="width:120px">下行带宽 &ge;</span><input id="alnet" type="number" step="any" min="0" value="'+(d.net||0)+'" style="width:84px"><span class="muted">B/s&nbsp;&nbsp;当前 '+fmtB(d.current.net)+'</span></div>'
    +'<div class="row"><span class="muted" style="width:120px">冷却时间</span><input id="alcooldown" type="number" min="60" value="'+(d.cooldown||900)+'" style="width:84px"><span class="muted">秒（防告警轰炸）</span></div>'
    +'<div style="margin-top:12px"><button class="pri" type="submit">保存配置</button></div>'
    +'</div>';
  h+='</form>';
  h+='<div class="card" style="margin-top:12px"><span class="muted">'+(on?'告警已开启':'告警已停用')+' · 上次发送: '+(d.last_sent?fmtTime(d.last_sent):'从未发送')+'</span></div>';
  $('view').innerHTML=h;
}).catch(function(){$('view').innerHTML='<span class="hot">无法连接 /api/alert</span>'})}
function alSave(e){e.preventDefault();post('/api/alert/save',{smtp_host:$('alhost').value.trim(),smtp_port:$('alport').value.trim(),smtp_user:$('aluser').value.trim(),smtp_pass:$('alpass').value,mode:$('almode').value,from:$('alfrom').value.trim(),to:$('alto').value.trim(),cpu:$('alcpu').value,mem:$('almem').value,disk:$('aldisk').value,net:$('alnet').value,cooldown:$('alcooldown').value},function(res){toast(res.msg||'已保存');loadAl()})}
function alEnable(on){post(on?'/api/alert/enable':'/api/alert/disable',{},function(res){toast(res.msg||(on?'已启用':'已停用'));loadAl()})}
function alTest(){post('/api/alert/test',{},function(res){toast((res.msg||'已发送')+(res.ok?'':'（失败）'))})}

function loadTk(){fetch('/api/tasks').then(function(r){return r.json()}).then(function(d){
  var h='<form class="rowform" onsubmit="tkAdd(event)"><input name="schedule" placeholder="cron 5 段，如 0 2 * * *" required style="width:220px"><input name="command" placeholder="执行命令，如 bash /opt/backup.sh" required style="flex:1;min-width:220px"><button class="pri" type="submit">+ 添加任务</button></form>';
  h+='<div class="card"><table><tr><th>调度</th><th>命令</th></tr>';
  (d.list||[]).forEach(function(t){h+='<tr><td><code>'+esc(t.schedule)+'</code></td><td>'+esc(t.command)+'</td></tr>'});
  h+='</table>'+muted((d.list||[]).length+' 条定时任务 (crontab)')+'</div>';
  $('view').innerHTML=h;
})}
function tkAdd(e){e.preventDefault();var sch=e.target.schedule.value.trim(),cmd=e.target.command.value.trim();if(!sch||!cmd)return;post('/api/tasks/add',{schedule:sch,command:cmd},function(res){toast(res.msg||'已添加');loadTk()})}

function post(path,fields,cb){var b=Object.keys(fields).map(function(k){return encodeURIComponent(k)+'='+encodeURIComponent(fields[k])}).join('&');fetch(path,{method:'POST',body:b,headers:{'Content-Type':'application/x-www-form-urlencoded'}}).then(function(r){return r.json()}).then(function(res){if(cb)cb(res);if(res&&res.ok===false&&!cb)toast(res.msg||'操作失败')})}

function loadShop(){fetch('/api/shop').then(function(r){return r.json()}).then(function(d){
  if(!d.ok){$('view').innerHTML='<div class="card" style="padding:30px;text-align:center"><div style="font-weight:700;margin-bottom:8px">软件商店 · 清单拉取失败</div><div class="muted">'+esc(d.msg||'未知错误')+'</div><div style="margin-top:16px"><button class="pri" onclick="loadShop()">重试</button></div></div>';return}
  var h='<div class="toolbar"><span class="muted" style="align-self:center">加速源: <code>'+esc(d.accel)+'</code></span><button class="pri" onclick="loadShop()">刷新清单</button></div>';
  h+='<div class="card"><table><tr><th>软件</th><th>说明</th><th></th></tr>';
  (d.list||[]).forEach(function(a){h+='<tr><td><b>'+esc(a.name)+'</b> <span class="muted">'+esc(a.id)+'</span></td><td>'+esc(a.desc)+'</td><td style="text-align:right"><button class="mini ok" onclick="shopInstall(\''+esc(a.id)+'\')">一键安装</button></td></tr>'});
  h+='</table>'+muted((d.list||[]).length+' 个软件 · 清单来自远程仓库，下载走加速')+'</div>';
  $('view').innerHTML=h;
})}
function shopInstall(id){if(!confirm('将下载并安装 '+id+' ，确定？'))return;var b=document.activeElement;b.disabled=true;b.textContent='安装中…';post('/api/shop/install',{id:id},function(res){
  b.disabled=false;b.textContent='一键安装';toast(res.msg||('安装请求已发出 '+id));
})}

function loadPlug(){
  var h='<div class="card" style="padding:6px 18px 18px"><div class="l" style="padding-top:14px">插件（极简 DSL + 微脚本语言）</div>'
    +'<div class="muted" style="line-height:1.8">在插件目录（默认 <code>plugins/</code>）放 <code>*.yml</code>；脚本支持 <code>if/else</code>、<code>for/while</code>、算术比较、<code>cmd()/fetch()/ret()/log()</code>、文本/数学函数、工具入参 <code>arg()</code> 与 KV 持久化 <code>kv_set()/kv_get()</code>。可从软件商店在线安装/更新/卸载。</div></div>'
    +'<div class="card" style="margin-top:14px"><div class="l">在线安装 <span class="muted">（来自 vp-store 仓库）</span></div><div id="plgstore" style="padding-top:6px"><button class="mini pri" onclick="loadPlugStore()">拉取商店清单</button></div></div>'
    +'<div class="card" style="margin-top:14px"><div class="l">已加载插件</div><div id="plglist" style="padding-top:6px">加载中…</div></div>'
    +'<div class="card" style="margin-top:14px"><div class="l">持久化 KV</div><div id="plgkv" style="padding-top:6px">—</div></div>'
    +'<div class="card" style="margin-top:14px"><div class="l">最近日志</div><div id="plglogs" style="padding-top:6px">—</div></div>';
  $('view').innerHTML=h;
  fetch('/api/plugins').then(function(r){return r.json()}).then(function(d){
    var el=$('plglist');
    if(!d.ok){el.innerHTML='<span class="hot">获取插件失败</span>';return}
    if(!(d.plugins||[]).length){el.innerHTML='<span class="muted">尚未加载任何插件</span>';}
    else{el.innerHTML='<table><tr><th>插件</th><th>版本</th><th>工具</th><th>定时</th><th></th></tr>'+(d.plugins||[]).map(function(p){
      var runs=(p.tools||[]).map(function(t){return '<button class="mini pri" onclick="plugRun(\''+esc(p.name)+'\',\''+esc(t.id)+'\',\''+esc(JSON.stringify(t.params||[]))+'\')">运行 '+esc(t.id)+'</button>'}).join(' ');
      var tg=p.enabled
        ?'<button class="mini" onclick="plugSwitch(\''+esc(p.name)+'\',\'disable\')">禁用</button>'
        :'<button class="mini" onclick="plugSwitch(\''+esc(p.name)+'\',\'enable\')">启用</button>';
      var un='<button class="mini danger" onclick="plugUninstall(\''+esc(p.name)+'\')">卸载</button>';
      return '<tr><td><b>'+esc(p.name)+'</b>'+(p.enabled?'':' <span class="hot">已禁用</span>')+'<div class="muted">'+esc(p.desc)+'</div></td><td class="muted">'+esc(p.version)+'</td>'
       +'<td>'+(p.tools||[]).map(function(t){return '<code>'+esc(t.id)+'</code>'}).join(' ')+'</td>'
       +'<td class="muted">'+(p.tasks||[]).map(function(t){return t.id+' /'+t.every+'s'}).join(' ')+'</td>'
       +'<td style="text-align:right">'+runs+' '+tg+' '+un+'</td></tr>'}).join('')+'</table>'+muted('钩子事件表：'+HOOKS.join(' · '));}
    var kv=$('plgkv');fetch('/api/plugin/kv').then(function(r){return r.json()}).then(function(k){kv.innerHTML=(k.kv||[]).map(function(x){return '<code>'+esc(x.k)+'</code> = <span>'+esc(x.v)+'</span>'}).join('<br>')||'<span class="muted">暂无 KV 数据</span>'});
    var lg=$('plglogs');
    lg.innerHTML=(d.logs||[]).length?'<pre style="white-space:pre-wrap;word-break:break-all;max-height:220px;overflow:auto;font-size:12px">'+(d.logs||[]).map(function(x){return esc(x)}).join('\n')+'</pre>':'<span class="muted">暂无日志</span>';
  }).catch(function(){var el=$('plglist');el.innerHTML='<span class="hot">无法连接 /api/plugins</span>'});
}
var HOOKS=['on_init','on_shutdown','on_tick','on_http_request','on_snapshot','on_process_list','on_service_start','on_service_stop','on_service_restart','on_firewall_allow','on_firewall_del','on_task_add','on_task_del','on_login','on_logout','on_shop_install','on_disk_low','on_cpu_high','on_mem_high','on_cron'];
function loadPlugStore(){
  var el=$('plgstore');el.innerHTML='<span class="muted">拉取中…</span>';
  fetch('/api/plugin/store').then(function(r){return r.json()}).then(function(d){
    if(!d.ok){el.innerHTML='<span class="hot">'+esc(d.msg||'拉取失败')+'</span>';return}
    if(!(d.list||[]).length){el.innerHTML='<span class="muted">商店暂无可用插件（需要 vp-store 仓库提供 plugins.yml）</span>';return}
    el.innerHTML='<table><tr><th>插件</th><th>说明</th><th></th></tr>'+(d.list||[]).map(function(i){
      return '<tr><td><b>'+esc(i.name)+'</b> <span class="muted">'+esc(i.id)+'</span></td><td>'+esc(i.desc)+'</td><td style="text-align:right"><button class="mini ok" onclick="plugInstall(\''+esc(i.id)+'\')">安装/更新</button></td></tr>'}).join('')+'</table>'+muted(d.mode==='builtin'?'当前为内置空清单，请确认网络/仓库可访问':'清单来源：vp-store');
  }).catch(function(){el.innerHTML='<span class="hot">无法连接 /api/plugin/store</span>'});
}
function plugInstall(id){if(!confirm('从商店安装/更新插件 '+id+' ？'))return;var b=document.activeElement;b.disabled=true;b.textContent='安装中…';post('/api/plugin/store/install',{id:id},function(res){b.disabled=false;b.textContent='安装/更新';toast(res.msg||('已请求安装 '+id));loadPlug();})}
function plugSwitch(name,on){post('/api/plugin/'+encodeURIComponent(name)+'/'+on,{},function(res){toast(res.msg||'已切换');loadPlug()})}
function plugUninstall(name){if(!confirm('确定卸载插件 '+name+' ？这会删除其清单文件。'))return;post('/api/plugin/'+encodeURIComponent(name)+'/uninstall',{},function(res){toast(res.msg||('已请求卸载 '+name));loadPlug()})}
function plugRun(plugin,tool,paramsJson){
  var params=paramsJson?JSON.parse(paramsJson):[];
  if(params.length){showForm(plugin,tool,params);return;}
  doRun(plugin,tool,{},document.activeElement);
}
function doRun(plugin,tool,fields,b){if(b){b.disabled=true;b.textContent='运行中…';}post('/api/plugin/'+encodeURIComponent(plugin)+'/'+encodeURIComponent(tool),fields,function(res){
  if(b){b.disabled=false;b.textContent='运行 '+tool;}
  toast(res.msg||('请求已发出 '+plugin+'/'+tool));
})}
function showForm(plugin,tool,params){
  var h='<div class="card"><div class="l">设置参数 · '+esc(plugin)+'/'+esc(tool)+'</div><div class="muted" style="line-height:1.6">该工具需要以下入参，脚本用 <code>arg("id")</code> 读取。</div><form class="rowform" style="flex-wrap:wrap" onsubmit="runForm(event,\''+esc(plugin)+'\',\''+esc(tool)+'\')">';
  params.forEach(function(p){
    h+='<label style="min-width:140px">'+esc(p.name||p.id)+(p.required?' <span class="hot">*</span>':'')+'</label>';
    var nm='name="'+esc(p.id)+'"';
    if(p.type==='select'){
      h+='<select '+nm+' style="flex:1;min-width:160px">'+(p.options||'').split(',').map(function(o){return '<option value="'+esc(o.trim())+'">'+esc(o.trim())+'</option>'}).join('')+'</select>';
    }else if(p.type==='bool'){
      h+='<select '+nm+' style="flex:1;min-width:160px"><option value="true">true</option><option value="false">false</option></select>';
    }else{
      h+='<input '+nm+' type="'+(p.type==='number'?'number':'text')+'" placeholder="'+esc(p.desc)+'" value="'+esc(p.default)+'" style="flex:1;min-width:160px">';
    }
    h+='<span class="muted" style="width:100%;font-size:12px">'+esc(p.desc)+'</span>';
  });
  h+='<button class="pri" type="submit">执行</button></form></div>';
  $('view').innerHTML=h;
}
function runForm(e,plugin,tool){e.preventDefault();var args={};var f=new FormData(e.target);f.forEach(function(v,k){args[k]=v});var s=$('view');s.innerHTML='<div class="card" style="padding:30px;text-align:center"><span class="muted">执行中…</span></div>';
  post('/api/plugin/'+encodeURIComponent(plugin)+'/'+encodeURIComponent(tool),args,function(res){toast(res.msg||'已执行');loadPlug();});
}

function loadMCP(){
  var h='<div class="card" style="padding:6px 18px 18px"><div class="l" style="padding-top:14px">连接地址（MCP Streamable HTTP）</div>'
    +'<code style="display:block;background:var(--line);padding:12px;border-radius:10px;margin:8px 0 14px">'+(location.protocol==='https:'?'https://':'http://')+location.host+'/mcp</code>'
    +'<div class="muted" style="line-height:1.8">在 Claude / Cursor 等 AI 客户端里添加 MCP 服务器并指向上述地址，即可让 AI 调用面板能力：系统监控、进程、服务、防火墙、定时任务。</div></div>'
    +'<div class="card" style="margin-top:14px"><div class="l">工具自检</div><div id="mcplist" style="padding-top:6px">正在测试…</div></div>'
    +'<div class="card" style="margin-top:14px"><form class="rowform" onsubmit="mcpTest(event)"><input name="tool" placeholder="工具名，如 system_overview / service_action" required style="flex:1;min-width:200px"><button class="pri" type="submit">测试调用</button></form></div>'
    +'<div class="card" style="margin-top:14px"><div class="l">返回结果</div><pre id="mcpout" style="white-space:pre-wrap;word-break:break-all;max-height:260px;overflow:auto;font-size:12px">—</pre></div>';
  $('view').innerHTML=h;
  fetch('/mcp',{method:'POST',headers:{'Content-Type':'application/json'},body:'{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'})
    .then(function(r){return r.json()}).then(function(d){
      var tools=(d.result&&d.result.tools)||[];var el=$('mcplist');
      el.innerHTML='<table><tr><th>工具</th><th>说明</th></tr>'+tools.map(function(t){return '<tr><td><code>'+esc(t.name)+'</code></td><td>'+esc(t.description)+'</td></tr>'}).join('')+'</table>'+muted('共 '+tools.length+' 个工具');
    }).catch(function(){var el=$('mcplist');el.innerHTML='<span class="hot">无法连接 /mcp 端点</span>'});
}
function mcpTest(e){e.preventDefault();var t=e.target.tool.value.trim();if(!t)return;
  var out=$('mcpout');out.textContent='调用 '+t+' …';
  var payload={jsonrpc:"2.0",id:2,method:"tools/call",params:{name:t,arguments:{}}};
  fetch('/mcp',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(payload)})
    .then(function(r){return r.json()}).then(function(d){
      var c=(d.result&&d.result.content||[]);out.textContent=c.map(function(x){return x.text}).join('\n')||JSON.stringify(d);
    }).catch(function(e2){out.textContent='调用失败: '+e2});
}
function muted(s){return '<div class="muted" style="padding:8px 2px 0">'+s+'</div>'}
function row(l,v,s){return '<tr><td class="muted" style="padding:9px 14px">'+l+'</td><td style="padding:9px 14px">'+v+'</td><td class="muted" style="padding:9px 14px">'+s+'</td></tr>'}
function fmtDate(t){if(!t)return '—';var d=new Date(t*1000);return d.getFullYear()+'-'+('0'+(d.getMonth()+1)).slice(-2)+'-'+('0'+d.getDate()).slice(-2)+' '+('0'+d.getHours()).slice(-2)+':'+('0'+d.getMinutes()).slice(-2)}

/* ---- 系统信息 ---- */
function loadInf(){
  fetch('/api/info').then(function(r){return r.json()}).then(function(d){
    var h='<div class="grid">'
      +stat('操作系统',esc(d.os||'—'),esc(d.kernel||'')+' · '+esc(d.arch||''))
      +stat('CPU',esc(d.cores)+' 核',esc(d.cpu_model||''))
      +stat('内存',fmtB(d.mem_used)+' / '+fmtB(d.mem_total),'使用率 '+d.mem_pct+'%')
      +stat('温度',esc(d.temp||'—'),'负载 '+esc(d.load||''))
      +'</div>';
    h+='<div class="card" style="margin-top:14px"><div class="l">详细</div><table>'
      +row('主机名',esc(d.host),'—')
      +row('运行时长',fmtUptime(d.uptime),'—')
      +row('交换分区',fmtB(d.swap_used)+' / '+fmtB(d.swap_total),'—')
      +'</table></div>';
    h+='<div class="card" style="margin-top:14px"><div class="l">磁盘分区</div><table><tr><th>文件系统</th><th>挂载点</th><th class="num">容量</th><th class="num">已用</th><th class="num">使用</th></tr>';
    (d.disks||[]).forEach(function(x){h+='<tr><td class="muted">'+esc(x.fs)+'</td><td>'+esc(x.mount)+'</td><td class="num">'+fmtB(x.total)+'</td><td class="num">'+fmtB(x.used)+'</td><td class="num">'+x.pct+'%</td></tr>'});
    h+='</table></div>';
    $('view').innerHTML=h;
  }).catch(function(){$('view').innerHTML='<span class="hot">无法获取系统信息</span>'});
}

/* ---- 网络连接 ---- */
function loadNet(){
  fetch('/api/conns').then(function(r){return r.json()}).then(function(d){
    var h='<div class="card"><div class="l">连接统计 <span class="muted">(ss/netstat)</span></div><table><tr><th>状态</th><th class="num">数量</th></tr>';
    (d.states||[]).forEach(function(s){h+='<tr><td>'+esc(s.state)+'</td><td class="num">'+s.count+'</td></tr>'});
    h+='</table>'+muted('合计 '+d.total+' 个连接')+'</div>';
    h+='<div class="card" style="margin-top:14px"><div class="l">本地端口 <span class="muted">(点击「结束」可终止监听进程)</span></div><table><tr><th>端口</th><th class="num">监听</th><th class="num">Established</th><th></th></tr>';
    (d.ports||[]).forEach(function(p){h+='<tr><td><code>'+esc(p.port)+'</code></td><td class="num">'+p.listen+'</td><td class="num">'+p.estab+'</td><td style="text-align:right">'+(p.listen>0?'<button class="mini danger" onclick="killConn(\''+esc(p.port)+'\')">结束</button>':'')+'</td></tr>'});
    h+='</table></div>';
    $('view').innerHTML=h;
  }).catch(function(){$('view').innerHTML='<span class="hot">无法获取连接信息</span>'});
}
function killConn(port){if(!confirm('确定结束监听端口 '+port+' 的进程？'))return;post('/api/conn/kill',{port:port},function(res){toast(res&&res.msg||'已请求');loadNet()})}

/* ---- 实时日志 ---- */
var LOGlIv=0, LOGlPos=0, LOGlFile='';
function loadLogCfg(){
  var h='<div class="card" style="padding:6px 18px 18px"><div class="l" style="padding-top:14px">实时日志（tail -f）</div>'
    +'<div class="muted" style="line-height:1.8">输入要跟随的日志文件绝对路径，面板每 2 秒增量拉取新增内容（不连续轮询，常驻内存不增）。</div></div>'
    +'<div class="card" style="margin-top:14px"><div class="l">选择/输入路径</div><div class="toolbar" style="margin-top:8px">'
    +'<input id="lgfile" list="lglist" placeholder="/var/log/syslog 或任意文件" style="flex:1" value="'+(LOGlFile||'')+'">'
    +'<datalist id="lglist">'+['/var/log/syslog','/var/log/messages','/var/log/nginx/access.log','/var/log/nginx/error.log','/var/log/auth.log','/var/log/dmesg','/tmp/vpanel.log'].map(function(x){return '<option value="'+x+'">'}).join('')+'</datalist>'
    +'<button class="pri" onclick="startLog()">跟随</button></div>'
    +'<div class="toolbar" style="margin-top:8px"><button class="mini" onclick="stopLog()">停止</button> <button class="mini" onclick="clearLogView()">清屏</button></div></div>'
    +'<div class="card" style="margin-top:14px"><div class="l">输出</div><pre id="lgout" style="white-space:pre-wrap;word-break:break-all;max-height:480px;overflow:auto;font-size:12px;background:var(--line);border-radius:10px;padding:12px;margin-top:8px">启动跟随后显示…</pre></div>';
  $('view').innerHTML=h;
  if(LOGlFile)startLog();
}
function startLog(){var f=($('lgfile')?$('lgfile').value:LOGlFile||'').trim();if(!f){toast('请先输入文件路径');return}LOGlFile=f;stopLog();
  fetch('/api/log/tail?file='+encodeURIComponent(f)+'&n=100').then(function(r){return r.json()}).then(function(d){
    if(!d.ok){$('lgout').textContent='读取失败: '+(d.msg||'');return}
    LOGlPos=d.size;var el=$('lgout');el.textContent=(d.lines||[]).join('\n');el.scrollTop=el.scrollHeight;
    LOGlIv=setInterval(function(){followLog()},2000);
  });
}
function followLog(){fetch('/api/log/follow?file='+encodeURIComponent(LOGlFile)+'&pos='+LOGlPos).then(function(r){return r.json()}).then(function(d){
    if(!d.ok){return}
    LOGlPos=d.size;var lines=d.lines||[];if(!lines.length)return;var el=$('lgout');if(!el)return;
    el.textContent=(el.textContent?el.textContent+'\n':'')+lines.join('\n');el.scrollTop=el.scrollHeight;
  });
}
function stopLog(){clearInterval(LOGlIv);LOGlIv=0;toast('已停止跟随')}
function clearLogView(){var el=$('lgout');if(el)el.textContent='';LOGlPos=0}

/* ---- 文件管理 ---- */
function loadFs(path){
  fetch('/api/files?path='+encodeURIComponent(path)).then(function(r){return r.json()}).then(function(d){
    if(!d.ok){$('view').innerHTML='<span class="hot">'+(d.msg||'无法读取目录')+'</span>';return}
    var cur=d.path||'/';LOGLCPATH=cur;
    var h='<div class="card" style="padding:6px 18px 18px"><div class="toolbar" style="padding-top:6px">'
      +'<code style="flex:1;background:var(--line);padding:10px 12px;border-radius:10px">'+esc(cur)+'</code>'
      +'<input type="file" id="upfile" style="display:none">'
      +'<button class="pri" onclick="pickUp()">上传到当前目录</button></div>'
      +'<div class="muted" style="margin-top:6px">点击目录进入，点击文件名查看/下载/删除。</div></div>';
    h+='<div class="card" style="margin-top:14px"><table><tr><th>名称</th><th class="num">大小</th><th>修改时间</th><th></th></tr>';
    if(cur!=='/'){
      var pp=cur.replace(/\/+$/,'');var idx=pp.lastIndexOf('/');var parent=idx>0?pp.slice(0,idx):'/';
      h+='<tr onclick="loadFs(\''+esc(parent)+'\')" style="cursor:pointer"><td>&#9650; 返回上级</td><td></td><td></td><td></td></tr>';
    }
    (d.list||[]).forEach(function(f){
      var fp=cur.replace(/\/+$/,'')+'/'+f.name;
      if(f.dir){
        h+='<tr onclick="loadFs(\''+esc(fp)+'\')" style="cursor:pointer"><td><span class="warm">&#128193;</span> '+esc(f.name)+'</td><td></td><td>'+fmtDate(f.mtime)+'</td><td style="text-align:right"><button class="mini danger" onclick="event.stopPropagation();fsDel(\''+esc(fp)+'\')">删除</button></td></tr>';
      }else{
        h+='<tr><td onclick="fsRead(\''+esc(fp)+'\')" style="cursor:pointer">'+esc(f.name)+'</td><td class="num">'+f.human+'</td><td>'+fmtDate(f.mtime)+'</td>'
         +'<td style="text-align:right"><button class="mini" onclick="fsDown(\''+esc(fp)+'\')">下载</button> <button class="mini danger" onclick="fsDel(\''+esc(fp)+'\')">删除</button></td></tr>';
      }
    });
    h+='</table>'+muted((d.list||[]).length+' 个条目')+'</div>';
    $('view').innerHTML=h;
  }).catch(function(){$('view').innerHTML='<span class="hot">无法连接 /api/files</span>'});
}
function pickUp(){var i=$('upfile');i.onchange=function(){var f=i.files[0];if(!f)return;uploadFile(LOGLCPATH,f)};i.click()}
var LOGLCPATH='/';
function uploadFile(dir,file){
  var target=dir.replace(/\/+$/,'')+'/'+file.name;
  var btn=document.querySelector('#view .pri');if(btn){btn.disabled=true;btn.textContent='上传中…'}
  fetch('/api/file/upload?path='+encodeURIComponent(target),{method:'POST',body:file})
    .then(function(r){return r.json()}).then(function(res){toast(res&&res.msg||('已上传 '+file.name));loadFs(dir)})
    .catch(function(){toast('上传失败')});
}
function fsRead(path){fetch('/api/file/read?path='+encodeURIComponent(path)).then(function(r){return r.json()}).then(function(d){
    if(!d.ok){toast(d.msg||'读取失败');return}
    var h='<div class="toolbar"><code style="flex:1;background:var(--line);padding:10px 12px;border-radius:10px">'+esc(d.name)+'</code>'
      +'<button class="mini" onclick="loadFs(\''+esc(parentDir(d.name))+'\')">返回目录</button>'
      +'<button class="pri" onclick="fsSave(\''+esc(d.name)+'\')">保存</button></div>';
    h+='<div class="card" style="margin-top:14px"><textarea id="fsed" style="width:100%;height:70vh;font-family:monospace;font-size:12px;border:1px solid var(--line);border-radius:10px;padding:12px">'+esc(d.data)+'</textarea></div>';
    $('view').innerHTML=h;
  }).catch(function(){toast('无法读取')});
}
function parentDir(p){var i=p.replace(/\/+$/,'').lastIndexOf('/');return i>0?p.slice(0,i):'/'}
function fsSave(path){var data=$('fsed').value;post('/api/file/save',{path:path,data:data},function(res){toast(res&&res.msg||'已保存');loadFs(parentDir(path))})}
function fsDel(path){if(!confirm('确定删除 '+path+' ？（目录会递归删除）'))return;post('/api/file/delete',{path:path},function(res){toast(res&&res.msg||'已删除');loadFs(parentDir(path))})}
function fsDown(path){window.location='/api/file/download?path='+encodeURIComponent(path)}

/* ---- 磁盘占用排行 ---- */
function loadDk(path){
  var h='<div class="card" style="padding:6px 18px 18px"><div class="l" style="padding-top:14px">磁盘占用排行（du）</div>'
    +'<div class="toolbar" style="margin-top:8px"><input id="dkpath" list="dklist" value="'+(path==='/'?'/':path)+'" style="flex:1">'
    +'<datalist id="dklist">'+['/','/root','/home','/var','/var/log','/usr','/opt','/data'].map(function(x){return '<option value="'+x+'">'}).join('')+'</datalist>'
    +'<button class="pri" onclick="doDk()">扫描</button></div>'
    +'<div class="muted" style="margin-top:6px">扫描该目录下第一级子目录占用大小（可能稍慢）。点击「去清理」跳转文件管理。</div></div>';
  h+='<div class="card" style="margin-top:14px" id="dkres"><div class="l">Top 占用</div><table><tr><th>路径</th><th class="num">大小</th><th></th></tr><tr><td colspan="3" class="muted">扫描中…</td></tr></table></div>';
  $('view').innerHTML=h;
  if(path)doDk();
}
function doDk(){var p=$('dkpath').value.trim()||'/';scanDk(p)}
function scanDk(path){
  var el=$('dkres');if(el)el.innerHTML='<div class="l">Top 占用（'+esc(path)+'）</div><table><tr><th>路径</th><th class="num">大小</th><th></th></tr><tr><td colspan="3" class="muted">扫描中…（du 可能耗时）</td></tr></table>';
  fetch('/api/disk/top?path='+encodeURIComponent(path)+'&n=25').then(function(r){return r.json()}).then(function(d){
    if(!d.ok){(el||$('view')).innerHTML='<span class="hot">'+(d.msg||'扫描失败')+'</span>';return}
    var h='<div class="l">Top 占用 · '+esc(path)+'</div><table><tr><th>路径</th><th class="num">大小</th><th></th></tr>';
    (d.list||[]).forEach(function(x){h+='<tr><td>'+esc(x.path)+'</td><td class="num">'+x.human+'</td><td style="text-align:right"><button class="mini" onclick="goClean(\''+esc(x.path)+'\')">去清理</button></td></tr>'});
    h+='</table>'+muted((d.list||[]).length+' 条');
    el.innerHTML=h;
  }).catch(function(){el.innerHTML='<span class="hot">扫描失败（du 异常）</span>'});
}
function goClean(p){cur='fs';renderTabs();loadFs(p)}

/* ---- 反向代理 / Nginx 站点 ---- */
function loadRp(){
  var h='<div class="card" style="padding:6px 18px 18px"><div class="l" style="padding-top:14px">反向代理 / Nginx 站点</div>'
    +'<div class="muted" style="line-height:1.8">管理 /etc/nginx 的 sites-available + sites-enabled（符号链接启用）。新增站点会先 <code>nginx -t</code> 校验，失败自动回滚。</div></div>'
    +'<div class="card" style="margin-top:14px"><div class="l">新增反代站点</div>'
    +'<form class="rowform" style="flex-wrap:wrap" onsubmit="rpAdd(event)">'
    +'<label style="min-width:90px">站点名 *<input name="name" style="flex:1" placeholder="myapp"></label>'
    +'<label style="min-width:90px">域名 *<input name="server_name" style="flex:1" placeholder="app.example.com"></label>'
    +'<label style="min-width:90px">端口 <input name="listen" style="flex:1" value="80"></label>'
    +'<label style="min-width:180px">代理到 *<input name="target" style="flex:1" placeholder="http://127.0.0.1:3000"></label>'
    +'<button class="pri" type="submit">创建并启用</button></form></div>'
    +'<div class="toolbar" style="margin-top:10px"><button class="mini" onclick="rpReload()">nginx reload</button> <button class="mini" onclick="loadRp()">刷新</button></div>';
  h+='<div class="card" style="margin-top:14px" id="rpres"><div class="l">已配置站点</div><table><tr><th>站点</th><th>监听</th><th>域名</th><th>代理到</th><th>状态</th><th></th></tr><tr><td colspan="6" class="muted">加载中…</td></tr></table></div>';
  $('view').innerHTML=h;
  fetch('/api/nginx').then(function(r){return r.json()}).then(function(d){
    var el=$('rpres');if(!d.ok){el.innerHTML='<span class="hot">'+d.msg+'</span>';return}
    var t='<div class="l">已配置站点 <span class="muted">('+esc(d.basedir)+')</span></div><table><tr><th>站点</th><th>监听</th><th>域名</th><th>代理到</th><th>状态</th><th></th></tr>';
    (d.list||[]).forEach(function(s){
      t+='<tr><td><b>'+esc(s.name)+'</b></td><td class="num">'+esc(s.listen||'—')+'</td><td>'+esc(s.server_name||'—')+'</td><td class="muted">'+esc(s.proxy_pass||'—')+'</td>'
       +'<td>'+(s.enabled?'<span class="cool">● 启用</span>':'<span class="warm">● 停用</span>')+'</td>'
       +'<td style="text-align:right">'
       +(s.enabled?'<button class="mini" onclick="rpToggle(\''+esc(s.name)+'\',false)">停用</button>':'<button class="mini ok" onclick="rpToggle(\''+esc(s.name)+'\',true)">启用</button>')
       +' <button class="mini danger" onclick="rpDel(\''+esc(s.name)+'\')">删除</button>'
       +'</td></tr>';
    });
    t+='</table>'+muted((d.list||[]).length+' 个站点');
    el.innerHTML=t;
  }).catch(function(){$('rpres').innerHTML='<span class="hot">无法连接 /api/nginx</span>'});
}
function rpAdd(e){e.preventDefault();var f=new FormData(e.target);var args={};f.forEach(function(v,k){args[k]=v});
  post('/api/nginx/add',args,function(res){toast(res&&res.msg||'已处理');if(res&&res.ok)loadRp()});}
function rpToggle(name,on){post('/api/nginx/toggle',{name:name,enable:(on?'true':'false')},function(res){toast(res&&res.msg||'已处理');loadRp()})}
function rpDel(name){if(!confirm('确定删除站点 '+name+'？（会删除配置文件）'))return;post('/api/nginx/delete',{name:name},function(res){toast(res&&res.msg||'已处理');loadRp()})}
function rpReload(){post('/api/nginx/reload',{},function(res){toast(res&&res.msg||'已 reload')})}

/* ---- 网站建站管理 ---- */
function loadWeb(){
  var h='<div class="card" style="padding:6px 18px 18px"><div class="l" style="padding-top:14px">网站建站（Nginx 站点 + 可选 PHP-FPM）</div>'
    +'<div class="muted" style="line-height:1.8">创建真实网站（自动建根目录与默认首页），可选接入 PHP-FPM；支持伪静态（WordPress/ThinkPHP/Laravel）与启停/删除。</div></div>'
    +'<div class="card" style="margin-top:14px"><div class="l">新建网站</div>'
    +'<form class="rowform" style="flex-wrap:wrap" onsubmit="webAdd(event)">'
    +'<label style="min-width:96px">站点名 *<input name="name" style="flex:1" placeholder="myblog"></label>'
    +'<label style="min-width:120px">域名 *<input name="domain" style="flex:1" placeholder="blog.example.com"></label>'
    +'<label style="min-width:72px">端口 <input name="listen" style="flex:1" value="80"></label>'
    +'<label style="min-width:88px" title="接入 PHP-FPM"><select name="php"><option value="0">静态</option><option value="1">PHP</option></select></label>'
    +'<button class="pri" type="submit">创建网站</button></form></div>'
    +'<div class="toolbar" style="margin-top:10px"><button class="mini" onclick="loadWeb()">刷新</button></div>';
  h+='<div class="card" style="margin-top:14px" id="webres"><div class="l">已配置网站</div><table><tr><th>站点</th><th>域名</th><th>端口</th><th>类型</th><th>根目录</th><th>状态</th><th></th></tr><tr><td colspan="7" class="muted">加载中…</td></tr></table></div>';
  $('view').innerHTML=h;
  fetch('/api/website').then(function(r){return r.json()}).then(function(d){
    var el=$('webres');if(!d.ok){el.innerHTML='<span class="hot">'+esc(d.msg||'获取失败')+'</span>';return}
    var t='<div class="l">已配置网站 <span class="muted">(root @ '+esc(d.basedir)+')</span></div><table><tr><th>站点</th><th>域名</th><th>端口</th><th>类型</th><th>根目录</th><th>状态</th><th></th></tr>';
    (d.list||[]).forEach(function(s){
      var php=s.php?'<span class="cool">PHP</span>':'<span class="muted">静态</span>';
      t+='<tr><td><b>'+esc(s.name)+'</b></td><td>'+esc(s.domain||'—')+'</td><td class="num">'+esc(s.listen||'—')+'</td><td>'+php+'</td><td class="muted">'+esc(s.root||'—')+'</td>'
       +'<td>'+(s.enabled?'<span class="cool">● 启用</span>':'<span class="warm">● 停用</span>')+'</td>'
       +'<td style="text-align:right;white-space:nowrap">'
       +'<select class="mini" onchange="webRew(\''+esc(s.name)+'\',this.value)"><option value="">伪静态…</option><option value="wordpress">WordPress</option><option value="thinkphp">ThinkPHP</option><option value="laravel">Laravel</option><option value="none">移除</option></select> '
       +(s.enabled?'<button class="mini" onclick="webTog(\''+esc(s.name)+'\',false)">停用</button>':'<button class="mini ok" onclick="webTog(\''+esc(s.name)+'\',true)">启用</button>')
       +' <button class="mini danger" onclick="webDel(\''+esc(s.name)+'\')">删除</button>'
       +'</td></tr>';
    });
    t+='</table>'+muted((d.list||[]).length+' 个网站');
    el.innerHTML=t;
  }).catch(function(){$('webres').innerHTML='<span class="hot">无法连接 /api/website</span>'});
}
function webAdd(e){e.preventDefault();var f=new FormData(e.target);var args={};f.forEach(function(v,k){args[k]=v});
  post('/api/website/create',args,function(res){toast(res&&res.msg||'已处理');if(res&&res.ok)loadWeb()});}
function webTog(name,on){post('/api/website/toggle',{name:name,enable:(on?'true':'false')},function(res){toast(res&&res.msg||'已处理');loadWeb()})}
function webDel(name){if(!confirm('确定删除网站 '+name+' ？勾选随包删除根目录。'))return;post('/api/website/delete',{name:name,drop_root:'false'},function(res){toast(res&&res.msg||'已处理');loadWeb()})}
function webRew(name,kind){if(kind==='')return;post('/api/website/rewrite',{name:name,kind:kind},function(res){toast(res&&res.msg||'已应用伪静态');loadWeb()})}

/* ---- 资源实时排行 + 开机自启 ---- */
function loadRs(){
  var h='<div class="card"><div class="l">CPU / 内存实时排行 <span class="muted">(top 式，采样 0.7s，每 5s 刷新)</span></div>'
    +'<div id="rsres"><table><tr><th class="num">CPU%</th><th>进程</th><th class="num">内存</th><th></th></tr><tr><td colspan="4" class="muted">采样中…</td></tr></table></div></div>';
  h+='<div class="card" style="margin-top:14px"><div class="l">开机自启服务 <span class="muted">(systemctl enabled)</span></div><div id="rsauto"><table><tr><th>服务</th><th>状态</th><th></th></tr><tr><td colspan="3" class="muted">加载中…</td></tr></table></div></div>';
  $('view').innerHTML=h;
  fetch('/api/top?n=25').then(function(r){return r.json()}).then(function(d){
    var t='<table><tr><th class="num">CPU%</th><th>进程</th><th class="num">内存</th><th></th></tr>';
    (d.list||[]).forEach(function(p){
      t+='<tr><td class="num">'+p.cpu+'%</td><td>'+esc(p.name)+' <span class="muted">(PID '+p.pid+')</span></td><td class="num">'+p.human+'</td><td style="text-align:right"><button class="mini danger" onclick="killP('+p.pid+')">结束</button></td></tr>'});
    t+='</table>'+(d.list||[]).length?'':muted('无数据');
    $('rsres').innerHTML=t;
  }).catch(function(){$('rsres').innerHTML='<span class="hot">采样失败</span>'});
  fetch('/api/autostart').then(function(r){return r.json()}).then(function(d){
    var t='<table><tr><th>服务</th><th>状态</th><th></th></tr>';
    (d.list||[]).forEach(function(s){
      t+='<tr><td>'+esc(s.name)+'</td><td class="muted">'+esc(s.state)+'</td><td style="text-align:right"><button class="mini danger" onclick="autoSet(\''+esc(s.name)+'\',false)">关闭自启</button></td></tr>'});
    t+='</table>'+(d.list||[]).length?'':muted('暂无可用的自启服务');
    $('rsauto').innerHTML=t;
  }).catch(function(){$('rsauto').innerHTML='<span class="hot">无法加载自启服务</span>'});
}
function autoSet(name,on){post('/api/autostart',{name:name,enable:(on?'true':'false')},function(res){toast(res&&res.msg||'已处理');loadRs()})}

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
  var st=document.getElementById('st');
  function setStatus(s){if(st)st.textContent=s}
  if(typeof Terminal==='undefined'||typeof FitAddon==='undefined'){
    setStatus('终端组件加载失败：需要联网拉取 xterm.js 依赖');return;
  }
  var term=new Terminal({cursorBlink:true,fontSize:14,scrollback:2000,bellStyle:'none'});
  var fit=new FitAddon.FitAddon();term.loadAddon(fit);term.open(document.getElementById('term'));fit.fit();
  var ws=new WebSocket((location.protocol==='https:'?'wss://':'ws://')+location.host+'/ws');
  var open=false, queue=[];
  var enc=new TextEncoder();
  term.onData(function(d){
    if(open){try{ws.send(enc.encode(d))}catch(e){}}
    else{queue.push(d)}
  });
  function sendSize(){
    if(open){try{ws.send('st\t'+term.cols+'\t'+term.rows)}catch(e){}}
  }
  ws.onopen=function(){
    open=true;setStatus('已连接 · '+term.cols+'x'+term.rows);term.focus();
    try{ws.send('st\t'+term.cols+'\t'+term.rows)}catch(e){}
    for(var i=0;i<queue.length;i++){try{ws.send(enc.encode(queue[i]))}catch(e){}}
    queue=[];
    setTimeout(function(){fit.fit()},50);
  };
  ws.onerror=function(){setStatus('连接失败');try{ws.close()}catch(e){}};
  ws.onclose=function(){setStatus('已断开');open=false;try{term.dispose()}catch(e){}};
  // PTY 输出是二进制帧：Blob/ArrayBuffer 都要转成 Uint8Array 才交给 xterm。
  ws.onmessage=function(ev){
    var d=ev.data;
    function write(u){try{term.write(u)}catch(e){}}
    if(d instanceof Blob){
      var fr=new FileReader();
      fr.onload=function(){write(new Uint8Array(fr.result))};
      fr.readAsArrayBuffer(d);
    } else if(d instanceof ArrayBuffer){
      write(new Uint8Array(d));
    } else {
      write(d);
    }
  };
  var rt=null;
  term.onResize(function(){clearTimeout(rt);rt=setTimeout(sendSize,100)});
  var r2=null;
  window.addEventListener('resize',function(){clearTimeout(r2);r2=setTimeout(function(){fit.fit();sendSize()},150)});
})();
</script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_attr_minimally_escapes() {
        assert_eq!(esc_attr("plain"), "plain");
        // 反斜杠优先转义。
        assert_eq!(esc_attr("a\\b"), "a\\\\b");
        // 双引号 -> &quot;。
        assert_eq!(esc_attr("say \"hi\""), "say &quot;hi&quot;");
        // 尖括号：只转义 `<`（> 不在转义白名单内）。
        assert_eq!(esc_attr("</script>"), "&lt;/script>");
        // 换行折叠为空格。
        assert_eq!(esc_attr("a\nb"), "a b");
    }

    #[test]
    fn rss_kb_reads_proc_self() {
        // Linux 下 /proc/self/status 必然存在，且常驻内存应 > 0。
        let kb = rss_kb();
        assert!(kb > 0);
    }
}