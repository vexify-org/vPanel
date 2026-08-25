//! 内置 MCP（Model Context Protocol）端点，供 AI 客户端调用面板能力。
//!
//! 采用 MCP Streamable HTTP 传输中最常用的一小部分：POST /mcp 上的 JSON-RPC
//! 方法 `initialize`、`tools/list`、`tools/call`。请求/响应均为 JSON-RPC 2.0，
//! 便于 Claude / Cursor 等客户端接入。
//!
//! tools/call 的参数从请求里的 `arguments:{...}` 对象中读取。

use crate::http::State;
use crate::json;

pub fn handle(body: &[u8], state: &State) -> Vec<u8> {
    let text = String::from_utf8_lossy(body);
    // id 可能同时作为 "id" 字段，统一定位。
    let method = str_field(&text, "method").unwrap_or("").to_string();
    let id_num = num_field(&text, "id");

    let resp = match method.as_str() {
        "initialize" => init_resp(id_num),
        "tools/list" => tools_list(id_num, state),
        "tools/call" => tools_call(&text, state, id_num),
        "ping" => fmt_jsonrpc(id_num, "{\"result\":{}}".into(), false),
        _ => fmt_jsonrpc(
            id_num,
            json::jesc("method not found").into(),
            true,
        ),
    };
    if resp.is_empty() {
        return Vec::new();
    }
    resp.into_bytes()
}

/// 拼装一个 JSON-RPC 响应。error=true 时返回 error 对象，否则 result。
fn fmt_jsonrpc(id: Option<i64>, payload: String, is_error: bool) -> String {
    let id = match id {
        Some(n) => n.to_string(),
        None => "null".to_string(),
    };
    let field = if is_error {
        format!("\"error\":{{\"code\":-32601,\"message\":\"{}\"}}", payload)
    } else {
        format!("\"result\":{}", payload)
    };
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{},{}}}", id, field)
}

fn init_resp(id: Option<i64>) -> String {
    let result = format!("{{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{{\"tools\":{{}}}},\"serverInfo\":{{\"name\":\"vpanel\",\"version\":\"{}\"}}}}", env!("CARGO_PKG_VERSION"));
    fmt_jsonrpc(
        id,
        result.to_string(),
        false,
    )
}

/// 工具清单。schema 用 r## 保留以便直接拼 JSON。
/// 集成插件工具：每个插件工具暴露为 `p_<plugin>.tool` 形式的 MCP 工具。
fn tools_list(id: Option<i64>, state: &State) -> String {
    let tools = [
        ("system_overview","查看系统状态：CPU、内存、磁盘、网络、负载","{}"),
        ("list_processes","列出进程（按内存前 80）","{}"),
        ("kill_process","结束进程","{\"pid\":{\"type\":\"number\"}}"),
        ("list_services","列出系统服务","{}"),
        ("service_action","启动/停止/重启服务，参数 name 与 action=start|stop|restart","{\"name\":{\"type\":\"string\"},\"action\":{\"type\":\"string\"}}"),
        ("list_firewall","列出防火墙放行规则","{}"),
        ("firewall_add","放行端口","{\"port\":{\"type\":\"string\"}}"),
        ("firewall_del","删除端口放行","{\"port\":{\"type\":\"string\"}}"),
        ("list_tasks","列出定时任务","{}"),
        ("task_add","添加定时任务，参数 schedule(5段cron) 与 command","{\"schedule\":{\"type\":\"string\"},\"command\":{\"type\":\"string\"}}"),
        ("system_info","查看系统信息：OS/内核/CPU型号/内存/磁盘分区/温度","{}"),
        ("list_conns","列出网络连接（TCP），含本地/远端地址与进程","{}"),
        ("kill_conn","结束占用某端口的连接","{\"port\":{\"type\":\"string\"}}"),
        ("list_files","列出目录文件（轻量，name/type/size/human）","{\"path\":{\"type\":\"string\"}}"),
        ("read_file","读取文本文件内容（按需读取）","{\"path\":{\"type\":\"string\"}}"),
        ("delete_path","删除文件或空目录","{\"path\":{\"type\":\"string\"}}"),
        ("write_file","写入文本文件（覆盖）","{\"path\":{\"type\":\"string\"},\"content\":{\"type\":\"string\"}}"),
        ("log_tail","查看文件尾部 n 行日志","{\"file\":{\"type\":\"string\"},\"n\":{\"type\":\"number\"}}"),
        ("disk_top","磁盘占用排行：扫描目录一级子目录占用，参数 path 与 n","{\"path\":{\"type\":\"string\"},\"n\":{\"type\":\"number\"}}"),
        ("list_nginx","列出反向代理/Nginx 站点","{}"),
        ("nginx_add","新增反向代理站点，参数 name/server_name/listen/target","{\"name\":{\"type\":\"string\"},\"server_name\":{\"type\":\"string\"},\"listen\":{\"type\":\"string\"},\"target\":{\"type\":\"string\"}}"),
        ("nginx_toggle","启用/停用反代站点，参数 name 与 enable(布尔)","{\"name\":{\"type\":\"string\"},\"enable\":{\"type\":\"boolean\"}}"),
        ("nginx_delete","删除反代站点，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("nginx_reload","重载 Nginx 配置","{}"),
        ("autostart","设置站点/服务开机自启，参数 name 与 enable(布尔)","{\"name\":{\"type\":\"string\"},\"enable\":{\"type\":\"boolean\"}}"),
        // ---- 网站（对标宝塔「网站」）----
        ("website_list","列出网站（域名/端口/根目录/PHP标记/启用状态）","{}"),
        ("website_create","创建网站，参数 name/domain/listen/php(布尔)/php_version","{\"name\":{\"type\":\"string\"},\"domain\":{\"type\":\"string\"},\"listen\":{\"type\":\"string\"},\"php\":{\"type\":\"boolean\"},\"php_version\":{\"type\":\"string\"}}"),
        ("website_toggle","启用/停用网站，参数 name 与 enable(布尔)","{\"name\":{\"type\":\"string\"},\"enable\":{\"type\":\"boolean\"}}"),
        ("website_delete","删除网站，参数 name 与 drop_root(布尔，是否连根目录一起删)","{\"name\":{\"type\":\"string\"},\"drop_root\":{\"type\":\"boolean\"}}"),
        ("website_rewrite","为站点应用伪静态规则，参数 name 与 kind(wordpress/thinkphp/laravel/none)","{\"name\":{\"type\":\"string\"},\"kind\":{\"type\":\"string\"}}"),
        // ---- 数据库（对标宝塔「数据库」）----
        ("db_status","查看数据库安装与运行状态","{}"),
        ("db_databases","列出所有数据库（排除系统库）","{}"),
        ("db_users","列出所有数据库账号（user/host）","{}"),
        ("db_backups","列出数据库备份文件","{}"),
        ("db_create_db","创建数据库，参数 name 与 charset(默认utf8mb4)","{\"name\":{\"type\":\"string\"},\"charset\":{\"type\":\"string\"}}"),
        ("db_drop_db","删除数据库，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("db_create_user","创建数据库账号，参数 user/pass/host","{\"user\":{\"type\":\"string\"},\"pass\":{\"type\":\"string\"},\"host\":{\"type\":\"string\"}}"),
        ("db_drop_user","删除数据库账号，参数 user/host","{\"user\":{\"type\":\"string\"},\"host\":{\"type\":\"string\"}}"),
        ("db_grant","将某库权限授予账号，参数 db/user/host","{\"db\":{\"type\":\"string\"},\"user\":{\"type\":\"string\"},\"host\":{\"type\":\"string\"}}"),
        ("db_backup","备份指定数据库，参数 db","{\"db\":{\"type\":\"string\"}}"),
        ("db_restore","从备份文件恢复数据库，参数 db/file","{\"db\":{\"type\":\"string\"},\"file\":{\"type\":\"string\"}}"),
        ("db_reset_root","重置 MySQL root 密码，参数 password","{\"password\":{\"type\":\"string\"}}"),
        // ---- SSL 证书（对标宝塔「SSL」）----
        ("ssl_list","列出 SSL 证书","{}"),
        ("ssl_import","导入证书，参数 name/fullchain/privkey","{\"name\":{\"type\":\"string\"},\"fullchain\":{\"type\":\"string\"},\"privkey\":{\"type\":\"string\"}}"),
        ("ssl_self_signed","生成自签证书，参数 name/domain/days","{\"name\":{\"type\":\"string\"},\"domain\":{\"type\":\"string\"},\"days\":{\"type\":\"number\"}}"),
        ("ssl_le_issue","申请 Let's Encrypt 证书，参数 name/domain/webroot","{\"name\":{\"type\":\"string\"},\"domain\":{\"type\":\"string\"},\"webroot\":{\"type\":\"string\"}}"),
        ("ssl_apply","为站点应用证书，参数 site/cert/upgrade(布尔)","{\"site\":{\"type\":\"string\"},\"cert\":{\"type\":\"string\"},\"upgrade\":{\"type\":\"boolean\"}}"),
        // ---- 运行环境（对标宝塔「软件商店/环境」）----
        ("env_status","查看运行环境总览（nginx/mysql/redis/php/node/docker/python/go）","{}"),
        ("env_install","一键安装运行时，参数 id(nginx/mysql/redis/php/node/docker/go)","{\"id\":{\"type\":\"string\"}}"),
        ("env_service","启停运行时服务，参数 id 与 action(start/stop/restart)","{\"id\":{\"type\":\"string\"},\"action\":{\"type\":\"string\"}}"),
        // ---- 备份（对标宝塔「定时备份」）----
        ("backup_list","列出备份文件","{}"),
        ("backup_dir","备份目录，参数 path 与 keep(保留份数)","{\"path\":{\"type\":\"string\"},\"keep\":{\"type\":\"number\"}}"),
        ("backup_run","执行全量备份（目录+数据库）","{}"),
        ("backup_schedule","设置定时备份，参数 cron(5段cron)","{\"cron\":{\"type\":\"string\"}}"),
        ("backup_schedule_remove","移除定时备份","{}"),
        ("backup_cloud","上传备份到云存储(FTP)，参数 file","{\"file\":{\"type\":\"string\"}}"),
        // ---- 安全（对标宝塔付费「安全/WAF」）----
        ("security_bans","列出已封禁 IP","{}"),
        ("security_hardening","查看系统加固状态（SSH）","{}"),
        ("security_waf_status","查看 WAF 状态","{}"),
        ("security_ban","封禁 IP，参数 ip","{\"ip\":{\"type\":\"string\"}}"),
        ("security_unban","解封 IP，参数 ip","{\"ip\":{\"type\":\"string\"}}"),
        ("security_brute","扫描暴力破解并自动封禁，参数 threshold(默认5)","{\"threshold\":{\"type\":\"number\"}}"),
        ("security_waf_enable","启用 WAF，参数 rps/burst","{\"rps\":{\"type\":\"number\"},\"burst\":{\"type\":\"number\"}}"),
        ("security_waf_disable","关闭 WAF","{}"),
        ("security_harden","SSH 加固，参数 no_root_pass/no_password(布尔)","{\"no_root_pass\":{\"type\":\"boolean\"},\"no_password\":{\"type\":\"boolean\"}}"),
        ("security_unharden","撤销 SSH 加固","{}"),
        // ---- IotaPanel 兼容插件（独立进程插件）----
        ("iota_list","列出 IotaPanel 兼容插件","{}"),
        ("iota_status","查看插件运行状态，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("iota_log","查看插件日志，参数 name 与 n(行数)","{\"name\":{\"type\":\"string\"},\"n\":{\"type\":\"number\"}}"),
        ("iota_start","启动插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("iota_stop","停止插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("iota_restart","重启插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("iota_uninstall","卸载插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("iota_keepalive","设置插件保活，参数 name 与 on(布尔)","{\"name\":{\"type\":\"string\"},\"on\":{\"type\":\"boolean\"}}"),
        ("iota_install_url","从 URL 安装插件，参数 url 与 sha256(可选)","{\"url\":{\"type\":\"string\"},\"sha256\":{\"type\":\"string\"}}"),
        // ---- HTTPS 反向代理网关（iotapanel https-front）----
        ("proxy_list","列出所有反向代理规则","{}"),
        ("proxy_add","添加反向代理规则，参数 prefix(/ 开头路径) 与 target(host:port)","{\"prefix\":{\"type\":\"string\"},\"target\":{\"type\":\"string\"}}"),
        ("proxy_del","删除反向代理规则，参数 prefix","{\"prefix\":{\"type\":\"string\"}}"),
        ("system_restart","重启面板自身（对齐 iotapanel /api/system/restart）","{}"),
        // ---- 插件商店 & KV ----
        ("plugin_store","列出插件商店软件","{}"),
        ("plugin_store_install","从商店安装插件，参数 id","{\"id\":{\"type\":\"string\"}}"),
        ("plugin_kv","列出插件 KV 存储","{}"),
        ("plugin_enable","启用插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("plugin_disable","禁用插件，参数 name","{\"name\":{\"type\":\"string\"}}"),
        // ---- 监控 / 资源排行 ----
        ("resource_top","资源占用排行（按 CPU/内存前 n），参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("monitor_snapshot","查看历史监控数据（最近 n 个采样点），参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("shop_list","列出软件商店应用","{}"),
        ("log_follow","增量查看日志，参数 file 与 pos(字节偏移)","{\"file\":{\"type\":\"string\"},\"pos\":{\"type\":\"number\"}}"),
        ("disk_usage","磁盘分区占用总览（df）","{}"),
        ("file_mkdir","创建目录，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("file_rename","重命名/移动文件，参数 src 与 dst","{\"src\":{\"type\":\"string\"},\"dst\":{\"type\":\"string\"}}"),
        ("docker_containers","列出所有 Docker 容器","{}"),
        ("docker_action","对容器执行操作，参数 id 与 action(start/stop/restart)","{\"id\":{\"type\":\"string\"},\"action\":{\"type\":\"string\"}}"),
        // ===== ops 运维工具箱（对标宝塔/类运维工具）=====
        // 网络诊断
        ("ping","Ping 探测主机，参数 host/count","{\"host\":{\"type\":\"string\"},\"count\":{\"type\":\"number\"}}"),
        ("tcp_ping","TCP 端口连通性探测，参数 host/port/count","{\"host\":{\"type\":\"string\"},\"port\":{\"type\":\"number\"},\"count\":{\"type\":\"number\"}}"),
        ("dns_lookup","DNS 解析查询，参数 host","{\"host\":{\"type\":\"string\"}}"),
        ("http_head","HTTP 响应头探测，参数 url","{\"url\":{\"type\":\"string\"}}"),
        ("listener_ports","本机所有监听端口(ss/netstat)","{}"),
        ("port_check","指定端口是否在监听，参数 port","{\"port\":{\"type\":\"number\"}}"),
        ("reverse_dns","反向解析 IP，参数 ip","{\"ip\":{\"type\":\"string\"}}"),
        // 系统纵深
        ("cpu_info","CPU 型号与核心数","{}"),
        ("cpu_usage","当前 CPU 使用率(采样1秒)","{}"),
        ("mem_info","内存总/已用/可用","{}"),
        ("swap_info","交换分区信息","{}"),
        ("loadavg","系统负载 1/5/15","{}"),
        ("net_io","网络吞吐 KB/s(采样1秒)","{}"),
        ("disk_inodes","磁盘 inode 使用(df -i)","{}"),
        ("os_release","操作系统发行版","{}"),
        ("kernel_info","内核/主机名/架构/运行时长","{}"),
        // 文件系统深度
        ("ls_long","长格式列出目录，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("dir_size","目录占用总大小，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("file_count","统计目录内文件数，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("file_search","递归搜索文件，参数 path(目录)/pattern","{\"path\":{\"type\":\"string\"},\"pattern\":{\"type\":\"string\"}}"),
        ("file_chmod","修改文件权限，参数 path/mode","{\"path\":{\"type\":\"string\"},\"mode\":{\"type\":\"string\"}}"),
        ("zip_archive","打包 tar.gz，参数 src/dst","{\"src\":{\"type\":\"string\"},\"dst\":{\"type\":\"string\"}}"),
        ("zip_extract","解压 tar.gz，参数 file/dest","{\"file\":{\"type\":\"string\"},\"dest\":{\"type\":\"string\"}}"),
        ("file_head","文件头部 N 行，参数 path/n","{\"path\":{\"type\":\"string\"},\"n\":{\"type\":\"number\"}}"),
        ("file_size","文件字节大小，参数 path","{\"path\":{\"type\":\"string\"}}"),
        // 进程
        ("process_by_name","按名字查找进程，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("process_detail","单进程详情，参数 pid","{\"pid\":{\"type\":\"number\"}}"),
        // 服务 / systemd
        ("systemd_units","列出 systemd 服务单元","{}"),
        ("systemd_action","对单元执行动作，参数 unit/action(start/stop/restart/reload/enable/disable)","{\"unit\":{\"type\":\"string\"},\"action\":{\"type\":\"string\"}}"),
        // 软件包(apt)
        ("apt_update","更新软件源索引","{}"),
        ("apt_upgrade","升级所有软件","{}"),
        ("apt_install","安装软件，参数 pkg","{\"pkg\":{\"type\":\"string\"}}"),
        ("apt_remove","移除软件，参数 pkg","{\"pkg\":{\"type\":\"string\"}}"),
        ("apt_list_installed","已安装软件列表(dpkg)","{}"),
        ("pkg_installed","软件是否已安装，参数 pkg","{\"pkg\":{\"type\":\"string\"}}"),
        // 计划任务
        ("cron_list","列出当前用户 crontab","{}"),
        ("cron_add","追加 cron 任务，参数 schedule/command","{\"schedule\":{\"type\":\"string\"},\"command\":{\"type\":\"string\"}}"),
        ("cron_remove","移除含关键字的 cron 行，参数 keyword","{\"keyword\":{\"type\":\"string\"}}"),
        ("cron_system","系统级 crontab 与 /etc/cron.d","{}"),
        // 运行时版本
        ("php_version","PHP 版本","{}"),
        ("node_version","Node.js 版本","{}"),
        ("go_version","Go 版本","{}"),
        ("python_version","Python 版本","{}"),
        ("mysql_version","MySQL 客户端版本","{}"),
        ("php_fpm_sockets","已安装的 PHP-FPM socket 列表","{}"),
        // 数据库深化
        ("db_sizes","各数据库大小(information_schema)","{}"),
        ("mysql_status","MySQL 运行状态(uptime/线程/版本)","{}"),
        ("mysql_ping","MySQL 连通性检查","{}"),
        // SSL 深化
        ("cert_view","查看证书明细(subject/issuer/有效期)，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("cert_expiry","证书剩余天数，参数 name","{\"name\":{\"type\":\"string\"}}"),
        // Docker 深化
        ("docker_images","列出 Docker 镜像","{}"),
        ("docker_stats","容器实时占用(CPU/内存)","{}"),
        ("docker_prune","清理未使用 Docker 资源","{}"),
        ("docker_info","Docker 版本信息","{}"),
        // 日志
        ("dmesg_tail","内核日志末尾，参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("journal_tail","systemd 单元日志，参数 unit/n","{\"unit\":{\"type\":\"string\"},\"n\":{\"type\":\"number\"}}"),
        ("nginx_error_tail","Nginx 错误日志末尾，参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("nginx_access_tail","Nginx 访问日志末尾，参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("mysql_error_tail","MySQL 错误日志末尾，参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("auth_log_tail","SSH 认证日志末尾，参数 n","{\"n\":{\"type\":\"number\"}}"),
        // 用户 / 杂项
        ("users_list","列出系统用户(/etc/passwd)","{}"),
        ("whoami","当前身份","{}"),
        ("random_password","生成随机强密码，参数 len","{\"len\":{\"type\":\"number\"}}"),
        ("sha256","计算 SHA-256，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("base64_encode","Base64 编码，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("base64_decode","Base64 解码，参数 enc","{\"enc\":{\"type\":\"string\"}}"),
        ("uuid","生成随机 UUID","{}"),
        ("panel_about","面板自述(版本/内存)","{}"),
        // ===== ops 九、安全 / 网络 / 系统实用 =====
        ("firewall_rules","防火墙规则(iptables/nftables)","{}"),
        ("net_interfaces","网卡地址表(ip addr)","{}"),
        ("route_table","内核路由表(ip route)","{}"),
        ("public_ip","公网 IPv4(回显服务)","{}"),
        ("uptime","系统运行时长","{}"),
        ("hostname","主机名","{}"),
        ("who_online","当前在线用户(who)","{}"),
        ("last_logins","最近登录记录(last)","{}"),
        ("cpu_per_core","每核心 CPU 使用率","{}"),
        ("zombie_count","僵尸进程计数","{}"),
        ("file_tail","查看文件末尾 N 行，参数 path/n","{\"path\":{\"type\":\"string\"},\"n\":{\"type\":\"number\"}}"),
        ("time_now","当前时间(epoch+UTC)","{}"),
        ("random_int","生成 [min,max] 随机整数，参数 min/max","{\"min\":{\"type\":\"number\"},\"max\":{\"type\":\"number\"}}"),
        ("read_env","读取环境变量，参数 name","{\"name\":{\"type\":\"string\"}}"),
        // ===== ops2 第二批 100 个工具（A 网络进阶）=====
        ("arp_table","ARP/邻居缓存表","{}"),
        ("dns_mx","查询 MX 记录，参数 host","{\"host\":{\"type\":\"string\"}}"),
        ("dns_ns","查询 NS 记录，参数 host","{\"host\":{\"type\":\"string\"}}"),
        ("dns_txt","查询 TXT 记录，参数 host","{\"host\":{\"type\":\"string\"}}"),
        ("traceroute_run","traceroute 路由跟踪，参数 host","{\"host\":{\"type\":\"string\"}}"),
        ("tcp_state_summary","TCP 各状态计数汇总","{}"),
        ("established_count","已建立连接数","{}"),
        ("listen_ipv6","IPv6 监听端口","{}"),
        ("gateway_ip","默认网关 IP","{}"),
        ("mac_by_ip","查询 IP 对应 MAC，参数 ip","{\"ip\":{\"type\":\"string\"}}"),
        // ===== ops2 B 系统纵深 =====
        ("os_version_id","OS 版本号(VERSION_ID)","{}"),
        ("arch","CPU 架构","{}"),
        ("core_count","CPU 核心数","{}"),
        ("context_switches","上下文切换累计","{}"),
        ("processes_count","累计创建进程数","{}"),
        ("processes_blocked","被阻塞进程数","{}"),
        ("processes_running","运行中进程数","{}"),
        ("boot_time","系统启动时刻(btime)","{}"),
        ("cache_mem","缓存内存","{}"),
        ("mem_available","可用内存","{}"),
        // ===== ops2 C 文件操作 =====
        ("file_stat","单文件详情(stat)，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("file_copy","复制文件/目录，参数 src/dst","{\"src\":{\"type\":\"string\"},\"dst\":{\"type\":\"string\"}}"),
        ("file_delete","递归删除，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("file_touch","创建空文件，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("file_append","追加文本到文件，参数 path/content","{\"path\":{\"type\":\"string\"},\"content\":{\"type\":\"string\"}}"),
        ("file_find","目录内按名找文件，参数 path/name","{\"path\":{\"type\":\"string\"},\"name\":{\"type\":\"string\"}}"),
        ("file_md5","文件 MD5，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("file_wc","文件行数/字节数，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("du_root","目录占用总大小，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("ln_symlink","创建软链接，参数 target/link","{\"target\":{\"type\":\"string\"},\"link\":{\"type\":\"string\"}}"),
        // ===== ops2 D 进程管理 =====
        ("process_tree","进程树(ps --forest)","{}"),
        ("process_threads_of","进程线程数，参数 pid","{\"pid\":{\"type\":\"number\"}}"),
        ("process_children_of","进程子进程列表，参数 pid","{\"pid\":{\"type\":\"number\"}}"),
        ("process_cwd","进程工作目录，参数 pid","{\"pid\":{\"type\":\"number\"}}"),
        ("process_cmdline","进程命令行，参数 pid","{\"pid\":{\"type\":\"number\"}}"),
        ("process_top_cpu","CPU 占用 TOP，参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("process_top_mem","内存占用 TOP，参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("process_state_count","进程各状态计数","{}"),
        ("kill_process_by_name","按名字结束进程，参数 name","{\"name\":{\"type\":\"string\"}}"),
        ("nice_set","调整进程优先级，参数 pid/nice","{\"pid\":{\"type\":\"number\"},\"nice\":{\"type\":\"number\"}}"),
        // ===== ops2 E 软件/包 =====
        ("apt_search","apt 搜索软件，参数 keyword","{\"keyword\":{\"type\":\"string\"}}"),
        ("apt_pkg_info","软件包详情，参数 pkg","{\"pkg\":{\"type\":\"string\"}}"),
        ("apt_depends","软件包依赖，参数 pkg","{\"pkg\":{\"type\":\"string\"}}"),
        ("dpkg_count","已安装包数量","{}"),
        ("pip_version","pip 版本","{}"),
        ("nginx_version","nginx 版本","{}"),
        ("redis_version","redis 版本","{}"),
        ("docker_version","docker 版本","{}"),
        ("git_version","git 版本","{}"),
        ("curl_version","curl 版本","{}"),
        // ===== ops2 F 服务/定时 =====
        ("systemd_failed","失败的 systemd 单元","{}"),
        ("systemd_enabled","已启用服务列表","{}"),
        ("systemd_timers","systemd 定时器","{}"),
        ("port_owner","占用某端口的进程，参数 port","{\"port\":{\"type\":\"number\"}}"),
        ("cron_full","完整 crontab(非注释)","{}"),
        ("at_jobs","at 定时任务","{}"),
        ("wanted_units","开机自启单元(multi-user)","{}"),
        ("login_sessions","当前登录会话(loginctl)","{}"),
        ("journal_size","journal 磁盘占用","{}"),
        ("tmp_count","/tmp 文件数","{}"),
        // ===== ops2 G 安全加固 =====
        ("uid0_users","UID 0 用户列表","{}"),
        ("sudoers_users","sudo/wheel 组成员","{}"),
        ("ssh_keys_present","存在 SSH 授权密钥的主机","{}"),
        ("ssh_param","SShd 配置参数值，参数 param","{\"param\":{\"type\":\"string\"}}"),
        ("open_ports_summary","监听端口计数汇总","{}"),
        ("pending_upgrades","待升级软件数","{}"),
        ("failed_auths","SSH 失败认证统计，参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("listening_uid_owners","监听端口与进程归属","{}"),
        ("mounts_with_exec","带 exec 挂载点","{}"),
        ("sensitive_perms","敏感文件权限(/etc/shadow等)","{}"),
        // ===== ops2 H 数据/编码 =====
        ("md5_digest","文本 MD5，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("sha1_digest","文本 SHA-1，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("cksum_text","文本 CRC(POSIX cksum)，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("url_encode","URL 百分号编码，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("url_decode","URL 百分号解码，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hex_encode","文本转十六进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hex_decode","十六进制转文本，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("base32_encode","Base32 编码，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("upper_case","转大写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("lower_case","转小写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        // ===== ops2 I 文本/处理 =====
        ("wc_lines","文件行数，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("wc_words","文件词数，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("grep_count","匹配行数，参数 path/pattern","{\"path\":{\"type\":\"string\"},\"pattern\":{\"type\":\"string\"}}"),
        ("grep_lines","匹配行(带行号)，参数 path/pattern","{\"path\":{\"type\":\"string\"},\"pattern\":{\"type\":\"string\"}}"),
        ("sort_numeric","数值排序，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unique_lines","去重排序，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("cut_field","按分隔符取第 field 列，参数 text/delim/field","{\"text\":{\"type\":\"string\"},\"delim\":{\"type\":\"string\"},\"field\":{\"type\":\"number\"}}"),
        ("tr_replace","字符串替换，参数 text/from/to","{\"text\":{\"type\":\"string\"},\"from\":{\"type\":\"string\"},\"to\":{\"type\":\"string\"}}"),
        ("append_line_once","文件无此行才追加，参数 path/line","{\"path\":{\"type\":\"string\"},\"line\":{\"type\":\"string\"}}"),
        ("csv_fields","按分隔符计数段数，参数 text/sep","{\"text\":{\"type\":\"string\"},\"sep\":{\"type\":\"string\"}}"),
        // ===== ops2 J 杂项/时间/校验 =====
        ("epoch_to_time","epoch 秒转 UTC 时间，参数 epoch","{\"epoch\":{\"type\":\"number\"}}"),
        ("timezone_offset","当前时区偏移(秒)","{}"),
        ("random_token","生成十六进制随机令牌，参数 len","{\"len\":{\"type\":\"number\"}}"),
        ("rand_bool","随机布尔(0/1)","{}"),
        ("valid_ip","校验 IPv4，参数 ip","{\"ip\":{\"type\":\"string\"}}"),
        ("valid_domain","校验域名，参数 host","{\"host\":{\"type\":\"string\"}}"),
        ("default_route_via","默认路由下一跳","{}"),
        ("dns_servers","DNS 服务器列表(/etc/resolv.conf)","{}"),
        ("swap_usage","交换分区使用","{}"),
        ("disk_io_simple","磁盘 IO(iostat/diskstats)","{}"),
        // ===== ops3 第三批 100 个工具（K-T 分类）=====
        // K 网络/流量
        ("iface_list","网卡接口列表","{}"),
        ("iface_speed","网卡速率 Mbps，参数 iface","{\"iface\":{\"type\":\"string\"}}"),
        ("iface_duplex","网卡双工模式，参数 iface","{\"iface\":{\"type\":\"string\"}}"),
        ("iface_mac","网卡 MAC 地址，参数 iface","{\"iface\":{\"type\":\"string\"}}"),
        ("iface_up","网卡是否 UP，参数 iface","{\"iface\":{\"type\":\"string\"}}"),
        ("traffic_since_boot","开机至今累计流量(/proc/net/dev)","{}"),
        ("udp_listen","UDP 监听端口","{}"),
        ("unix_sockets","UNIX 域套接字","{}"),
        ("ip_v6_addr","IPv6 地址列表","{}"),
        ("ping_loss","Ping 丢包率汇总，参数 host","{\"host\":{\"type\":\"string\"}}"),
        // L 系统/内核
        ("kernel_release","内核发布版本","{}"),
        ("kernel_version","内核完整版本","{}"),
        ("hostname_full","主机名","{}"),
        ("kernel_config","读取 kernel sysctl 参数，参数 param","{\"param\":{\"type\":\"string\"}}"),
        ("vm_params","VM 虚拟内存参数(sysctl)","{}"),
        ("fs_params","文件系统参数(nr_open/file-nr)","{}"),
        ("net_params","网络内核参数","{}"),
        ("entropy_avail","可用熵值","{}"),
        ("allowed_ports","本机可用端口区间","{}"),
        ("mem_zones","内存页信息(zoneinfo)","{}"),
        // M 磁盘/挂载
        ("mount_list","挂载点列表","{}"),
        ("mount_by_point","按挂载点查询，参数 point","{\"point\":{\"type\":\"string\"}}"),
        ("disk_uuid","磁盘 UUID(blkid/lsblk)","{}"),
        ("disk_model","磁盘型号列表","{}"),
        ("disk_readonly","只读挂载点","{}"),
        ("inode_usage","指定路径 inode 使用，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("fs_type","文件系统类型，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("block_devices","块设备列表","{}"),
        ("sector_size","磁盘扇区大小","{}"),
        ("swap_devices","交换设备列表","{}"),
        // N 用户/权限
        ("user_home","用户主目录，参数 user","{\"user\":{\"type\":\"string\"}}"),
        ("user_shell","用户登录 Shell，参数 user","{\"user\":{\"type\":\"string\"}}"),
        ("user_groups","用户所属组，参数 user","{\"user\":{\"type\":\"string\"}}"),
        ("group_members","组成员列表，参数 group","{\"group\":{\"type\":\"string\"}}"),
        ("user_last_login","用户最近登录，参数 user","{\"user\":{\"type\":\"string\"}}"),
        ("lock_users","被锁定用户列表","{}"),
        ("logins_total","累计登录次数","{}"),
        ("nologin_users","nologin 用户列表","{}"),
        ("file_owner","文件属主:属组，参数 path","{\"path\":{\"type\":\"string\"}}"),
        ("effective_uid","当前身份(id)","{}"),
        // O 安全/加固
        ("selinux_status","SELinux 状态","{}"),
        ("apparmor_status","AppArmor 状态","{}"),
        ("world_writable","全写文件列表","{}"),
        ("suid_bins","SUID 二进制列表","{}"),
        ("socket_perms","关键 Socket 权限","{}"),
        ("ip_forward","IP 转发开关","{}"),
        ("firewall_active","防火墙活动状态","{}"),
        ("listen_low_ports","<1024 低端口监听","{}"),
        ("umask_current","当前 umask","{}"),
        ("core_pattern","core dump 模式","{}"),
        // P 文本/变换
        ("char_count","字符/字节计数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_reverse","字符串反转，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("word_count","词数统计，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_empty","是否空白串，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("has_digits","是否含数字，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("has_upper","是否含大写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("dashed_line","60 个横线分隔符","{}"),
        ("repeat_str","重复字符串，参数 text/n","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"number\"}}"),
        ("title_case","转标题大小写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("swap_case","大小写互换，参数 text","{\"text\":{\"type\":\"string\"}}"),
        // Q 数学/统计
        ("sum_list","求和，参数 text(空格分隔)","{\"text\":{\"type\":\"string\"}}"),
        ("avg_list","求均值，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("min_max","最小/最大值，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("median","中位数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_prime","判断质数，参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("factorial","阶乘，参数 n","{\"n\":{\"type\":\"number\"}}"),
        ("gcd","最大公约数，参数 a/b","{\"a\":{\"type\":\"number\"},\"b\":{\"type\":\"number\"}}"),
        ("lcm","最小公倍数，参数 a/b","{\"a\":{\"type\":\"number\"},\"b\":{\"type\":\"number\"}}"),
        ("power","幂运算，参数 base/exp","{\"base\":{\"type\":\"number\"},\"exp\":{\"type\":\"number\"}}"),
        ("percentage","百分比占比，参数 text","{\"text\":{\"type\":\"string\"}}"),
        // R 时间/日期
        ("uptime_seconds","运行秒数","{}"),
        ("utc_now","当前 UTC(epoch/约年)","{}"),
        ("iso_date","当前 UTC 日期时间","{}"),
        ("weekday","当前星期几","{}"),
        ("quarter","当前季度","{}"),
        ("seconds_until","距目标时间剩余秒，参数 end_unix","{\"end_unix\":{\"type\":\"number\"}}"),
        ("calendar_seed","毫秒时间戳种子","{}"),
        ("is_leap_year","是否闰年，参数 year","{\"year\":{\"type\":\"number\"}}"),
        ("day_count_month","当月天数","{}"),
        ("time_signed_bin","UNIX 时间戳","{}"),
        // S 进程/系统字段
        ("pid_count_all","当前进程数","{}"),
        ("process_start","进程启动时间，参数 pid","{\"pid\":{\"type\":\"number\"}}"),
        ("process_rss","进程常驻内存，参数 pid","{\"pid\":{\"type\":\"number\"}}"),
        ("process_vsz","进程虚拟内存，参数 pid","{\"pid\":{\"type\":\"number\"}}"),
        ("process_state","进程状态，参数 pid","{\"pid\":{\"type\":\"number\"}}"),
        ("open_files","进程打开文件数，参数 pid","{\"pid\":{\"type\":\"number\"}}"),
        ("io_by_pid","进程 IO 统计，参数 pid","{\"pid\":{\"type\":\"number\"}}"),
        ("longest_cmdline","命令行最长进程","{}"),
        ("thread_total","线程总数","{}"),
        ("dir_nlink","目录子项数(nlink)，参数 path","{\"path\":{\"type\":\"string\"}}"),
        // T 杂项/实用
        ("tar_list","列出 tar.gz 文件内容，参数 file","{\"file\":{\"type\":\"string\"}}"),
        ("gz_info","gzip 文件压缩信息，参数 file","{\"file\":{\"type\":\"string\"}}"),
        ("sha256sum_file","文件 SHA-256，参数 file","{\"file\":{\"type\":\"string\"}}"),
        ("env_all","全部环境变量","{}"),
        ("echo_args","回显参数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("len_bytes","字符串字节数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_numeric","字符串是否数值，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("to_int","字符串转整数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("byte_units","字节转人类可读，参数 bytes","{\"bytes\":{\"type\":\"number\"}}"),
        ("is_systemd","是否为 systemd 系统","{}"),
        // ==== ops4 数学与单位换算 ====
        ("is_whole","是否为整数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_positive","是否正数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_negative","是否负数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_even","是否偶数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_odd","是否奇数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_integer_range","是否在32位整数范围，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("floor_num","向下取整，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ceil_num","向上取整，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("trunc_num","截断小数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("round_num","四舍五入，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("abs_num","绝对值，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("sign_num","符号(+/-/0)，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("negate_num","取相反数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("add_three","三数相加，参数 a b c","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"},\"c\":{\"type\":\"string\"}}"),
        ("mul_three","三数相乘，参数 a b c","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"},\"c\":{\"type\":\"string\"}}"),
        ("sub2","两数相减，参数 a b","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("div2","两数相除，参数 a b","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("rem2","两数取余，参数 a b","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("avg2","两数平均，参数 a b","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("avg3","三数平均，参数 a b c","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"},\"c\":{\"type\":\"string\"}}"),
        ("square_num","平方，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("cube_num","立方，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("sqrt_num","平方根，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("inv_num","倒数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("pow2","2的n次方，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("pow10x","10的n次方，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("log10_num","常用对数log10，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("log2_num","以2为底对数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ln_num","自然对数ln，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("exp_num","自然指数e^x，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("sin_deg","正弦(角度)，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("cos_deg","余弦(角度)，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("tan_deg","正切(角度)，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("rad2deg","弧度转角度，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("deg2rad","角度转弧度，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("fib_num","斐波那契第n项，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("triangular_num","三角数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("digit_sum","各位数字之和，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("digit_count","数字位数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("collatz_steps","考拉兹步数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_prime2","是否素数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("next_prime","不小于n的素数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("circle_area","圆面积，参数 text(半径)","{\"text\":{\"type\":\"string\"}}"),
        ("circle_circumference","圆周长，参数 text(半径)","{\"text\":{\"type\":\"string\"}}"),
        ("sphere_volume","球体积，参数 text(半径)","{\"text\":{\"type\":\"string\"}}"),
        ("pythagoras","勾股斜边，参数 a b","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("rect_perimeter","矩形周长，参数 a b","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("rect_area","矩形面积，参数 a b","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("c2f","摄氏转华氏，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("f2c","华氏转摄氏，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("c2k","摄氏转开尔文，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("k2c","开尔文转摄氏，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("bytes_to_kb","字节转KB，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("bytes_to_mb","字节转MB，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("bytes_to_gb","字节转GB，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("gb_to_bytes","GB转字节，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("mb_to_kb","MB转KB，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("mbps_to_mbper_s","Mbps转MB/s，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("km_to_miles","公里转英里，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("miles_to_km","英里转公里，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("cm_to_m","厘米转米，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("m_to_km","米转公里，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("kg_to_lb","千克转磅，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hz_to_khz","Hz转kHz，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("percent_of","占比百分比，参数 a(部分) b(总量)","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("random_range","0~(n-1)随机数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("digits_of_pi","圆周率若干位，参数 text","{\"text\":{\"type\":\"string\"}}"),
        // ==== ops5 字符串与文本处理 ====
        ("str_len","字符个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_char_at","取第 i 个字符，参数 text i","{\"text\":{\"type\":\"string\"},\"i\":{\"type\":\"string\"}}"),
        ("str_slice","按索引 i..j 截取子串，参数 text i j","{\"text\":{\"type\":\"string\"},\"i\":{\"type\":\"string\"},\"j\":{\"type\":\"string\"}}"),
        ("str_first","取首字符，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_last","取末字符，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_first_n","取前 n 个字符，参数 text n","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"string\"}}"),
        ("str_last_n","取后 n 个字符，参数 text n","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"string\"}}"),
        ("str_upper","转全大写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_lower","转全小写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_title","标题式每词首字母大写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_swap_case","大小写互换，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_capitalize","句首大写其余原样，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_sentence","整句规范化：句首大写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_trim","去首尾空白，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_trim_start","去开头空白，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_trim_end","去结尾空白，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_ltrim_char","去开头指定字符，参数 text ch","{\"text\":{\"type\":\"string\"},\"ch\":{\"type\":\"string\"}}"),
        ("str_rtrim_char","去结尾指定字符，参数 text ch","{\"text\":{\"type\":\"string\"},\"ch\":{\"type\":\"string\"}}"),
        ("str_trim_char","去首尾指定字符，参数 text ch","{\"text\":{\"type\":\"string\"},\"ch\":{\"type\":\"string\"}}"),
        ("str_pad_left","左填充到长度 n，参数 text n ch","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"string\"},\"ch\":{\"type\":\"string\"}}"),
        ("str_pad_right","右填充到长度 n，参数 text n ch","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"string\"},\"ch\":{\"type\":\"string\"}}"),
        ("str_zfill","数字左补0到长度 n，参数 text n","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"string\"}}"),
        ("str_center","居中对齐到长度 n，参数 text n ch","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"string\"},\"ch\":{\"type\":\"string\"}}"),
        ("str_truncate","截断到 n 字符并加省略号，参数 text n","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"string\"}}"),
        ("str_contains","是否包含子串，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("str_starts_with","是否以某前缀开头，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("str_ends_with","是否以某后缀结尾，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("str_index_of","首次出现索引(无则-1)，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("str_last_index","末次出现索引(无则-1)，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("str_count","子串出现次数，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("str_replace","替换首个匹配，参数 text a b","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("str_remove","移除首个匹配，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("str_remove_all","移除全部匹配，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("str_delete","删除索引 i..j 段，参数 text i j","{\"text\":{\"type\":\"string\"},\"i\":{\"type\":\"string\"},\"j\":{\"type\":\"string\"}}"),
        ("str_insert","在索引 at 处插入，参数 text at b","{\"text\":{\"type\":\"string\"},\"at\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("str_repeat","重复 n 次，参数 text n","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"string\"}}"),
        ("str_rev","反转字符串，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_split","按分隔符拆成数组，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("str_join","以 sep 连接 a 与 b，参数 a sep b","{\"a\":{\"type\":\"string\"},\"sep\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("str_word_count","单词个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_line_count","行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_number_lines","为每行加行号，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_first_word","取首个单词，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_last_word","取末个单词，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_alpha_count","字母个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_digit_count","数字字符个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_space_count","空白字符个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_punct_count","标点符号个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_vowel_count","元音字母个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_unique_chars","去重字符序列，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_is_empty","是否为空串，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_is_digits","是否纯数字，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_is_letters","是否纯字母，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_is_alnum","是否字母数字，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_is_upper","是否全大写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_is_lower","是否全小写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_is_space","是否全空白，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_is_palindrome","是否回文，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_to_snake","转 snake_case，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_to_kebab","转 kebab-case，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_to_camel","转 camelCase，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_to_pascal","转 PascalCase，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_edit_distance","编辑距离(莱文斯坦)，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("str_similarity","相似度0~1，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("str_quote","加双引号，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_unquote","去首尾引号，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_escape","转义换行/制表/反斜杠，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_unescape","还原转义序列，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("str_indent","每行前加 n 空格，参数 text n","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"string\"}}"),
        // ==== ops6 编码/校验和/进制 ====
        ("dec2bin","十进制转二进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("dec2oct","十进制转八进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("dec2hex","十进制转十六进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("dec2hexu","十进制转大写十六进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("bin2dec","二进制转十进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("oct2dec","八进制转十进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hex2dec","十六进制转十进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("bin2oct","二进制转八进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("bin2hex","二进制转十六进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("oct2bin","八进制转二进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("oct2hex","八进制转十六进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hex2bin","十六进制转二进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("dec2base","十进制转任意进制(2-36)，参数 text base","{\"text\":{\"type\":\"string\"},\"base\":{\"type\":\"string\"}}"),
        ("base2dec","任意进制转十进制，参数 text base","{\"text\":{\"type\":\"string\"},\"base\":{\"type\":\"string\"}}"),
        ("dec2roman","十进制转罗马数字，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("roman2dec","罗马数字转十进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("sum_bytes","所有字节值之和，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("xor_checksum","逐字节异或校验，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("djb2_hash","djb2 哈希(十进制)，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("djb2_hash_hex","djb2 哈希(十六进制)，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("sdbm_hash","sdbm 哈希，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("fnv1a32_hash","FNV-1a 32位哈希，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("fnv1a64_hash","FNV-1a 64位哈希，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("adler32_cksum","Adler-32 校验和，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("crc32_cksum","CRC-32 校验和，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hamming_weight","字符串中 1 的个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("count_ones","整数的置位位数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("parity_bit","整数的奇偶校验位，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("bit_length","整数的二进制位数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_power_of_two","是否为2的幂，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("utf8_hex","文本UTF-8字节的十六进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hex_decode_text","十六进制还原为文本，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("char_code","首字符的Unicode码位，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("char_code_hex","首字符码位的十六进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("utf8_len","UTF-8字节长度，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("b64_encode_text","Base64 编码，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("b64_decode_text","Base64 解码，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("b64url_encode_text","URL安全 Base64 编码，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("b64url_decode_text","URL安全 Base64 解码，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("rot13","ROT13 字母旋转，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("caesar_shift","凯撒移位，参数 text shift","{\"text\":{\"type\":\"string\"},\"shift\":{\"type\":\"string\"}}"),
        ("xor_cipher","与 key 逐字节异或，参数 text key","{\"text\":{\"type\":\"string\"},\"key\":{\"type\":\"string\"}}"),
        ("swap_bytes","字节逆序输出十六进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        // ==== ops7 时间与日期 ====
        ("now_unix","当前Unix秒，参数 无","{}"),
        ("now_millis","当前Unix毫秒，参数 无","{}"),
        ("now_date","当前日期YYYY-MM-DD，参数 无","{}"),
        ("now_time","当前时间HH:MM:SS，参数 无","{}"),
        ("now_datetime","当前完整时间，参数 无","{}"),
        ("now_weekday","当前中文明天名，参数 无","{}"),
        ("now_dow","当前星期几(1-7)，参数 无","{}"),
        ("now_iso","当前ISO8601时间，参数 无","{}"),
        ("now_uptime","系统已运行秒数，参数 无","{}"),
        ("unix_to_date","Unix秒转日期，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unix_to_time","Unix秒转时间，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unix_to_datetime","Unix秒转完整时间，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unix_to_weekday","Unix秒转多月名，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unix_year","Unix秒取年份，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unix_month","Unix秒取月份，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unix_day","Unix秒取日，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unix_hour","Unix秒取小时，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unix_minute","Unix秒取分钟，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unix_second","Unix秒取秒钟，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unix_doy","Unix秒取年内第几天，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("unix_dow","Unix秒取星期几，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_leap_yr","是否闰年，参数 text(年份)","{\"text\":{\"type\":\"string\"}}"),
        ("days_in_m","某月天数，参数 text(YYYY-MM)","{\"text\":{\"type\":\"string\"}}"),
        ("days_in_yr","某年天数，参数 text(年份)","{\"text\":{\"type\":\"string\"}}"),
        ("date_dow","日期是星期几，参数 text(YYYY-MM-DD)","{\"text\":{\"type\":\"string\"}}"),
        ("date_weekday","日期中文明名，参数 text(YYYY-MM-DD)","{\"text\":{\"type\":\"string\"}}"),
        ("date_doy","日期年内第几天，参数 text(YYYY-MM-DD)","{\"text\":{\"type\":\"string\"}}"),
        ("is_weekend","是否周末，参数 text(YYYY-MM-DD)","{\"text\":{\"type\":\"string\"}}"),
        ("week_of_year","年内第几周，参数 text(YYYY-MM-DD)","{\"text\":{\"type\":\"string\"}}"),
        ("is_month_first","是否当月1日，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_month_last","是否当月最后一天，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("month_name","月份中文名，参数 text(1-12)","{\"text\":{\"type\":\"string\"}}"),
        ("month_days_txt","某月总体天数，参数 text(1-12)","{\"text\":{\"type\":\"string\"}}"),
        ("date_to_unix_stamp","日期转Unix秒，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("add_days","日期加N天，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("add_hours","日期加N小时，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("add_minutes","日期加N分钟，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("add_months","日期加N月，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("add_years","日期加N年，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("date_diff_days","两日期相差天数，参数 a b","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("date_diff_seconds","两日期相差秒数，参数 a b","{\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("seconds_to_hms","秒转HH:MM:SS，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("minutes_to_hms","分转HH:MM:SS，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hours_to_days","小时转天+小时，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("seconds_to_human","秒转可读时长，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ms_to_seconds","毫秒转秒，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("days_to_years","天数转年，参数 text","{\"text\":{\"type\":\"string\"}}"),
        // ==== ops8 网络/地址/端口 ====
        ("ip_valid_v4","是否合法IPv4，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_octets","IPv4四段数组，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_octet_at","取IPv4某段，参数 text i","{\"text\":{\"type\":\"string\"},\"i\":{\"type\":\"string\"}}"),
        ("ip_to_int","IPv4转32位整数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_from_int","整数转IPv4，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_increment","IPv4加1，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_decrement","IPv4减1，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_is_private","是否私有IP，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_is_loopback","是否回环地址，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_is_multicast","是否组播地址，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_is_link_local","是否链路本地地址，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_class","IP地址类别A-E，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_reverse","IP字节逆序，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("mask_from_prefix","前缀长度转掩码，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("prefix_from_mask","掩码转前缀长度，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("subnet_network","子网网络地址，参数 text a(前缀)","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("subnet_broadcast","子网广播地址，参数 text a(前缀)","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("subnet_hosts","子网可用主机数，参数 text a(前缀)","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("cidr_contains","IP是否在CIDR内，参数 text(cidr) a(ip)","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("cidr_first_ip","CIDR起始IP，参数 text(cidr)","{\"text\":{\"type\":\"string\"}}"),
        ("cidr_last_ip","CIDR结束IP，参数 text(cidr)","{\"text\":{\"type\":\"string\"}}"),
        ("cidr_size","CIDR地址总数，参数 text(cidr)","{\"text\":{\"type\":\"string\"}}"),
        ("ipv6_valid","是否合法IPv6，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ipv6_groups","IPv6段个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("mac_valid","是否合法MAC，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("mac_colon","MAC统一冒号格式，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("mac_is_unicast","是否单播MAC，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("mac_is_multicast","是否组播MAC，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("port_valid","是否合法端口，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("port_service","端口常见服务名，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("port_class","端口类别，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("port_range_count","端口区间数量，参数 text(n-m)","{\"text\":{\"type\":\"string\"}}"),
        ("port_in_range","端口是否在区间，参数 text(n-m) a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("domain_valid","域名格式校验，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("domain_labels","域名标签数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("domain_tld","域名顶级域，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("domain_has_www","是否以www开头，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("email_valid","邮箱格式校验，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("email_local","邮箱本地部分，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("email_domain","邮箱域名部分，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("http_status_name","HTTP状态码含义，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("http_status_class","HTTP状态码类别，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_2xx","是否2xx状态，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_4xx","是否4xx状态，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_5xx","是否5xx状态，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_compare","比较两IP大小，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("ip_is_broadcast","是否广播地址，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_is_zero","是否0.0.0.0，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ip_wildcard_mask","IP通配掩码，参数 text","{\"text\":{\"type\":\"string\"}}"),
        // ---- ops9 文件/路径/权限/磁盘/进程 ----
        ("path_basename","路径文件名，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_dirname","路径所在目录，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_ext","路径扩展名，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_ext_set","替换扩展名，参数 text e","{\"text\":{\"type\":\"string\"},\"e\":{\"type\":\"string\"}}"),
        ("path_stem","文件名去扩展名，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_join","拼接两级路径，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("path_normalize","规范化路径，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_clean","清理重复斜杠，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_is_abs","是否绝对路径，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_is_rel","是否相对路径，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_depth","路径层级数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_split","路径分段数组，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_parent_all","逐级父目录数组，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_common_prefix","公共前缀路径，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("path_is_within","是否位于目录内，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("path_relativize","求相对路径，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("path_ensure_slash","末尾补斜杠，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_trim_slash","去掉首尾斜杠，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_sep_count","斜杠个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_home_expand","~展开为绝对路径，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_is_root","是否根路径 /，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_regex_escape","转义路径正则字符，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_indexed","取第n层路径段，参数 text n","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"string\"}}"),
        ("path_tail_n","取末尾n段路径，参数 text n","{\"text\":{\"type\":\"string\"},\"n\":{\"type\":\"string\"}}"),
        ("path_has_hidden","是否含隐藏文件，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("path_double_dots","统计..次数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("perm_rwx","八进制转rwx串，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("perm_octal","rwx串转八进制，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("perm_symbol_sum","rwx权值之和，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("perm_sticky","检查特殊权限位，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("perm_like","掩码包含判定，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("perm_extended","含特殊位的rwx串，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("perm_world","其他位权限判定，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("size_in_bytes","可读大小转字节，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("size_auto","字节转自动单位，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("size_compare","比较两个大小，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("size_bits_units","逐单位换算展示，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("size_ratio","两大小比值，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("disk_fill_percent","磁盘使用百分比，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("disk_free_est","磁盘剩余估算，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("block_count","字节按块取整，参数 text block","{\"text\":{\"type\":\"string\"},\"block\":{\"type\":\"string\"}}"),
        ("text_line_count","文本行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_bytes","文本字节长度，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_words","文本单词数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_nonws","非空白字符数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_ntabs","制表符数量，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_nlines_nl","换行符数量，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_line_len","最长行字符数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_contains_cn","是否含中文，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_ascii_percent","ASCII字符占比，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_ident_lines","缩进行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_blank_lines","空行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_average_wlen","平均词长，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_synopsis","文本摘要前40字，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_longest_line","最长行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("text_tabs_to_spaces","制表符转空格，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("proc_cpu_percent","CPU占用百分比，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("proc_vm_human","进程内存转可读，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("load_interpret","负载值中文释义，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("uptime_days","运行秒转天数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("proc_state_desc","进程状态字母释义，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("proc_user_ratio","用户态CPU占比，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("mem_percent_free","可用内存评估，参数 text","{\"text\":{\"type\":\"string\"}}"),
        // ---- ops10 安全/颜色/数论/日志 ----
        ("entropy_of","字符串信息熵，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("sec_charset_size","不同字符数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("pass_class_count","密码字符类别数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("pass_has_upper","是否含大写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("pass_has_lower","是否含小写，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("pass_has_digit","是否含数字，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("pass_has_special","是否含特殊字符，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("pass_strength","密码强度评级，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("pass_estimate_bits","密码熵估值(位)，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("pass_common_weak","是否常见弱密码，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("html_escape","HTML实体转义，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("html_unescape","HTML实体反转义，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("uri_scheme","URI协议名，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("uri_host","URI主机名，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("uri_segments","URI路径分段，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hex_to_rgb","十六进制颜色转RGB，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("rgb_to_hex","RGB转十六进制颜色，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("is_hex","是否合法颜色，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hex_brightness","颜色感知亮度，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hex_is_light","是否亮色，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hex_complement","互补色，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hex_lighten","颜色提亮，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("hex_darken","颜色压暗，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("hex_contrast","两色对比度，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("hex_blend","两色混合，参数 text a r","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"},\"r\":{\"type\":\"string\"}}"),
        ("math_fact","阶乘，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("math_gcd","最大公约数，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("math_lcm","最小公倍数，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("math_modpow","模幂运算，参数 text a m","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"},\"m\":{\"type\":\"string\"}}"),
        ("math_is_prime","是否素数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("math_next_prime","下一素数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("math_num_divisors","因子个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("math_digital_root","数字根，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("math_is_perfect","是否完全数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("math_nthfib","第n个斐波那契数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("math_is_coprime","是否互质，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("math_triangle_type","三角形类型，参数 text a b","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("log_level","日志级别判定，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("log_ip_count","日志去重IP数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("log_error_lines","错误行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("log_warn_lines","警告行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("log_info_lines","INFO行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("log_ts_count","含时间戳行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("log_stacktrace_lines","堆栈帧行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("log_line_lengths","日志行长统计，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("cfg_comment_lines","注释行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("cfg_brace_balance","花括号平衡值，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("cfg_section_count","配置段数量，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("cfg_equals_count","含等号行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("json_balanced","JSON括号是否平衡，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("sec_control_chars","控制字符数量，参数 text","{\"text\":{\"type\":\"string\"}}"),
        // ---- ops11 统计/哈希/频次/文本距离/业务/换算 ----
        ("stat_sum","数列求和，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("stat_min","数列最小值，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("stat_max","数列最大值，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("stat_mean","数列平均值，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("stat_median","数列中位数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("stat_mode","数列众数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("stat_range","数列极差，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("stat_variance","数列方差，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("stat_stdev","数列标准差，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("stat_geomean","几何平均数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("stat_peak_to_avg","峰值均值比，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("value_size","数值个数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hash_fnv1a","FNV-1a哈希，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hash_djb2","djb2哈希，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hash_elf","ELF哈希，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("hash_adler","Adler-32校验，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("freq_char_count","指定字符出现次数，参数 text ch","{\"text\":{\"type\":\"string\"},\"ch\":{\"type\":\"string\"}}"),
        ("freq_top_chars","出现最多的前5字符，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("letter_freq","A-Z字母频次，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("ngram_distinct","不同n元组个数，参数 text size","{\"text\":{\"type\":\"string\"},\"size\":{\"type\":\"string\"}}"),
        ("levenshtein_dist","编辑距离，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("hamming_dist","汉明距离，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("palin_check","回文检测，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("anagram_check","变位词检测，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("lcs_length","最长公共子序列长度，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("dist_manhattan","曼哈顿距离，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("geo_haversine","两经纬度距离(km)，参数 text a b c","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"},\"c\":{\"type\":\"string\"}}"),
        ("percent_change","变化百分比，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("compound_growth","复利终值，参数 text a b","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("discount_price","折扣后价格，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("tip_split","小费分摊，参数 text a b","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("bmi_calc","体重指数BMI，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("loan_payment","等额本息月供，参数 text a b","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"},\"b\":{\"type\":\"string\"}}"),
        ("kelvin_to_c","开尔文转摄氏，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("c_to_kelvin","摄氏转开尔文，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("kmh_to_mph","公里时转英里时，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("mph_to_kmh","英里时转公里时，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("days_between_dates","两日期天数差，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("day_of_year","一年中第几天，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("month_days_of","某月天数，参数 text a","{\"text\":{\"type\":\"string\"},\"a\":{\"type\":\"string\"}}"),
        ("weekday_of","日期星期几，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("csv_row_count","CSV数据行数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("csv_first_cols","CSV首行列数，参数 text","{\"text\":{\"type\":\"string\"}}"),
        ("delim_repeat","分隔符出现次数，参数 text delim","{\"text\":{\"type\":\"string\"},\"delim\":{\"type\":\"string\"}}"),
    ];
    let mut item = Vec::new();
    for (name, desc, schema) in tools {
        item.push(format!(
            "{{\"name\":\"{}\",\"description\":\"{}\",\"inputSchema\":{{\"type\":\"object\",\"properties\":{}}}}}",
            name,
            json::jesc(desc),
            schema
        ));
    }
    // 追加插件工具（依据工具 params 生成 inputSchema）。
    for p in plugins_snapshot(state) {
        for t in &p.tools {
            let mut s = String::new();
            s.push_str("{\"name\":\"");
            s.push_str(&plugin_tool_name(&p, &t.id));
            s.push_str("\",\"description\":\"[插件 ");
            s.push_str(&p.name);
            s.push_str("] ");
            s.push_str(&json::jesc(&t.desc));
            // 由 params 生成 properties 对象。
            let mut props = Vec::new();
            for pp in &t.params {
                let ty = match pp.r#type.as_str() {
                    "number" => "number",
                    "bool" => "boolean",
                    _ => "string",
                };
                props.push(format!(
                    "\"{}\":{{\"type\":\"{}\",\"description\":\"{}\"}}",
                    json::jesc(&pp.id),
                    ty,
                    json::jesc(if pp.desc.is_empty() { &pp.name } else { &pp.desc })
                ));
            }
            s.push_str(&format!(
                "\",\"inputSchema\":{{\"type\":\"object\",\"properties\":{{{}}}}}",
                props.join(",")
            ));
            s.push('}');
            item.push(s);
        }
    }
    let result = format!(
        "{{\"tools\":[{}]}}",
        item.join(",")
    );
    fmt_jsonrpc(id, result, false)
}

/// tools/call：执行工具，参数从 arguments 对象读取。
fn tools_call(body: &str, state: &State, id: Option<i64>) -> String {
    // MCP tools/call: params.name 是工具名；params.arguments 是参数。
    // 由于 params 出现在 arguments 之前，首个 "name" 即工具名。
    let name = str_field(body, "name").unwrap_or("").to_string();
    // 解析 arguments 对象里的各字段。
    let args = extract_arg_json(body);
    let pid = arg_str(&args, "pid");
    let svc_name = arg_str(&args, "name");
    let action = arg_str(&args, "action");
    let port = arg_str(&args, "port");
    let schedule = arg_str(&args, "schedule");
    let command = arg_str(&args, "command");
    let path = arg_str(&args, "path");
    let content = arg_str(&args, "content");
    let file = arg_str(&args, "file");
    let n: usize = arg_str(&args, "n").parse().unwrap_or(20);
    let pos: u64 = arg_str(&args, "pos").parse().unwrap_or(0);
    // 文件操作 / 容器 / 磁盘
    let src = arg_str(&args, "src");
    let dst = arg_str(&args, "dst");
    // ops 工具箱（host/url/ip/schedule 已在下方其它分组声明，避免遮蔽）
    let count: u32 = arg_str(&args, "count").parse().unwrap_or(0);
    let tport: u32 = arg_str(&args, "port").parse().unwrap_or(0);
    let mode = arg_str(&args, "mode");
    let pattern = arg_str(&args, "pattern");
    let unit = arg_str(&args, "unit");
    let pkg = arg_str(&args, "pkg");
    let keyword = arg_str(&args, "keyword");
    let tpid: u32 = arg_str(&args, "pid").parse().unwrap_or(0);
    let dest = arg_str(&args, "dest");
    let len: u32 = arg_str(&args, "len").parse().unwrap_or(0);
    let tlines: usize = arg_str(&args, "n").parse().unwrap_or(50);
    let min: i64 = arg_str(&args, "min").parse().unwrap_or(0);
    let max: i64 = arg_str(&args, "max").parse().unwrap_or(100);
    let text = arg_str(&args, "text");
    let enc = arg_str(&args, "enc");
    let server_name = arg_str(&args, "server_name");
    let listen = arg_str(&args, "listen");
    let target = arg_str(&args, "target");
    let fname = arg_str(&args, "name");
    let enable = arg_str(&args, "enable") == "true" || arg_str(&args, "enable") == "1";
    // 网站
    let domain = arg_str(&args, "domain");
    let php = arg_str(&args, "php") == "true" || arg_str(&args, "php") == "1";
    let php_version = arg_str(&args, "php_version");
    let drop_root = arg_str(&args, "drop_root") == "true" || arg_str(&args, "drop_root") == "1";
    let kind = arg_str(&args, "kind");
    // 数据库
    let dbname = arg_str(&args, "name");
    let charset = arg_str(&args, "charset");
    let dbuser = arg_str(&args, "user");
    let dbpass = arg_str(&args, "pass");
    let host = arg_str(&args, "host");
    let db = arg_str(&args, "db");
    let password = arg_str(&args, "password");
    // SSL
    let fullchain = arg_str(&args, "fullchain");
    let privkey = arg_str(&args, "privkey");
    let cert = arg_str(&args, "cert");
    let site = arg_str(&args, "site");
    let days: u32 = arg_str(&args, "days").parse().unwrap_or(365);
    let webroot = arg_str(&args, "webroot");
    let upgrade = arg_str(&args, "upgrade") == "true" || arg_str(&args, "upgrade") == "1";
    // 环境 / 备份 / 安全
    let env_id = arg_str(&args, "id");
    let cron = arg_str(&args, "cron");
    // cron 与 file 已复用上面 command/file 变量？file 单独解析：
    // 备份目录 path 复用 path；keep
    let keep: u32 = arg_str(&args, "keep").parse().unwrap_or(5);
    // 安全
    let ip = arg_str(&args, "ip");
    let threshold: u32 = arg_str(&args, "threshold").parse().unwrap_or(5);
    let rps: u32 = arg_str(&args, "rps").parse().unwrap_or(20);
    let burst: u32 = arg_str(&args, "burst").parse().unwrap_or(40);
    let no_root_pass = arg_str(&args, "no_root_pass") == "true" || arg_str(&args, "no_root_pass") == "1";
    let no_password = arg_str(&args, "no_password") == "true" || arg_str(&args, "no_password") == "1";
    // iota
    let url = arg_str(&args, "url");
    let sha256 = arg_str(&args, "sha256");
    // 反向代理
    let prefix = arg_str(&args, "prefix");
    // ops2 / ops3 新增参数
    let iface = arg_str(&args, "iface");
    let point = arg_str(&args, "point");
    let grp = arg_str(&args, "group");
    let usr = arg_str(&args, "user");
    let param = arg_str(&args, "param");
    let link = arg_str(&args, "link");
    let end_unix: i64 = arg_str(&args, "end_unix").parse().unwrap_or(0);
    let year: i64 = arg_str(&args, "year").parse().unwrap_or(2024);
    let a: u64 = arg_str(&args, "a").parse().unwrap_or(0);
    let b: u64 = arg_str(&args, "b").parse().unwrap_or(0);
    let base: u64 = arg_str(&args, "base").parse().unwrap_or(0);
    let exp: u32 = arg_str(&args, "exp").parse().unwrap_or(0);
    let bytes: u64 = arg_str(&args, "bytes").parse().unwrap_or(0);
    // ops2 H/I/J 文本与数学参数
    let nice: i32 = arg_str(&args, "nice").parse().unwrap_or(0);
    let delim = arg_str(&args, "delim");
    let field: u32 = arg_str(&args, "field").parse().unwrap_or(1);
    let from = arg_str(&args, "from");
    let to = arg_str(&args, "to");
    let sep = arg_str(&args, "sep");
    let c = arg_str(&args, "c");
    let epoch: i64 = arg_str(&args, "epoch").parse().unwrap_or(0);
    // ops4/ops5：需要原始字符串形式的 a/b/i/j/at/ch
    let sa = arg_str(&args, "a");
    let sb = arg_str(&args, "b");
    let si = arg_str(&args, "i");
    let sj = arg_str(&args, "j");
    let s_at = arg_str(&args, "at");
    let s_ch = arg_str(&args, "ch");
    let sn = arg_str(&args, "n");
    let key = arg_str(&args, "key");
    let shift = arg_str(&args, "shift");
    let base_s = arg_str(&args, "base");
    let sm = arg_str(&args, "m");
    let sr = arg_str(&args, "r");
    let sc = arg_str(&args, "c");
    let s_size = arg_str(&args, "size");
    let s_delim = arg_str(&args, "delim");

    let (ok, text) = match name.as_str() {
        "system_overview" => (true, crate::system::system_json(&state.monitor)),
        "list_processes" => (true, crate::system::processes_json()),
        "list_services" => (true, crate::ctl::services_json()),
        "list_firewall" => (true, crate::ctl::firewall_json()),
        "list_tasks" => (true, crate::ctl::tasks_json()),
        "service_action" => crate::ctl::service_action(&svc_name, &action),
        "firewall_add" => {
            let (o, m) = crate::ctl::fw_allow(&port);
            (o, m)
        }
        "firewall_del" => {
            let (o, m) = crate::ctl::fw_delete(&port);
            (o, m)
        }
        "task_add" => {
            let (o, m) = crate::ctl::task_add(&schedule, &command);
            (o, m)
        }
        "kill_process" => match pid.trim().parse::<u32>() {
            Ok(p) => (crate::system::kill_pid(p), format!("请求结束进程 {}", p)),
            Err(_) => (false, "pid 需为数字".into()),
        },
        "system_info" => (true, crate::extra::sysinfo_json()),
        "list_conns" => (true, crate::extra::conns_json()),
        "kill_conn" => {
            let (o, m) = crate::extra::conn_kill(&port);
            (o, m)
        }
        "list_files" => {
            let p = if path.is_empty() { "/".to_string() } else { path.clone() };
            (true, crate::extra::ls_json(&p))
        }
        "read_file" => (true, crate::extra::read_file_json(&path)),
        "delete_path" => {
            let (o, m) = crate::extra::del_path(&path);
            (o, m)
        }
        "write_file" => {
            let (o, m) = crate::extra::write_file(&path, content.as_bytes());
            (o, m)
        }
        "log_tail" => {
            let p = if file.is_empty() { "/var/log/syslog".to_string() } else { file.clone() };
            (true, crate::extra::log_tail_json(&p, n))
        }
        "disk_top" => {
            let p = if path.is_empty() { "/".to_string() } else { path.clone() };
            (true, crate::extra::disk_top_json(&p, n))
        }
        "list_nginx" => (true, crate::nginx::nginx_list_json()),
        "nginx_add" => {
            let (o, m) = crate::nginx::nginx_add(&fname, &server_name, &listen, &target);
            (o, m)
        }
        "nginx_toggle" => {
            let (o, m) = crate::nginx::nginx_toggle(&fname, enable);
            (o, m)
        }
        "nginx_delete" => {
            let (o, m) = crate::nginx::nginx_delete(&fname);
            (o, m)
        }
        "nginx_reload" => crate::nginx::nginx_reload_endpoint(),
        "autostart" => crate::nginx::autostart_action(&fname, enable),
        // ---- 网站 ----
        "website_list" => (true, crate::website::website_list_json()),
        "website_create" => {
            let (o, m) = crate::website::website_create(&fname, &domain, &listen, php, &php_version);
            (o, m)
        }
        "website_toggle" => {
            let (o, m) = crate::website::website_toggle(&fname, enable);
            (o, m)
        }
        "website_delete" => {
            let (o, m) = crate::website::website_delete(&fname, drop_root);
            (o, m)
        }
        "website_rewrite" => {
            let (o, m) = crate::website::rewrite_apply(&fname, &kind);
            (o, m)
        }
        // ---- 数据库 ----
        "db_status" => {
            let installed = crate::db::installed(&state.cfg.database);
            let running = installed && crate::db::server_running(&state.cfg.database);
            (true, format!(
                "{{\"ok\":true,\"installed\":{},\"running\":{},\"user\":\"{}\"}}",
                installed, running, json::jesc(&state.cfg.database.user)
            ))
        }
        "db_databases" => {
            let (o, m) = crate::db::databases(&state.cfg.database);
            (o, format!("{{\"ok\":{},\"list\":{}}}", o, m))
        }
        "db_users" => {
            let (o, m) = crate::db::users(&state.cfg.database);
            (o, format!("{{\"ok\":{},\"list\":{}}}", o, m))
        }
        "db_backups" => (true, db_backups_text(&state.cfg)),
        "db_create_db" => crate::db::create_db(&state.cfg.database, &dbname, &charset),
        "db_drop_db" => crate::db::drop_db(&state.cfg.database, &dbname),
        "db_create_user" => crate::db::create_user(&state.cfg.database, &dbuser, &dbpass, &host),
        "db_drop_user" => crate::db::drop_user(&state.cfg.database, &dbuser, &host),
        "db_grant" => crate::db::grant(&state.cfg.database, &db, &dbuser, &host),
        "db_backup" => crate::db::backup(&state.cfg.database, &db, &state.cfg.database.backup_dir),
        "db_restore" => crate::db::restore(&state.cfg.database, &db, &file),
        "db_reset_root" => crate::db::reset_root_password(&state.cfg.database, &password),
        // ---- SSL ----
        "ssl_list" => (true, crate::ssl::list_json(&state.cfg.certs)),
        "ssl_import" => crate::ssl::import(&state.cfg.certs, &fname, &fullchain, &privkey),
        "ssl_self_signed" => crate::ssl::self_signed(&state.cfg.certs, &fname, &domain, days),
        "ssl_le_issue" => crate::ssl::le_issue(&state.cfg.certs, &fname, &domain, &webroot),
        "ssl_apply" => crate::ssl::apply(&state.cfg.certs, &site, &cert, upgrade),
        // ---- 运行环境 ----
        "env_status" => (true, crate::env::status_json()),
        "env_install" => crate::env::install(&env_id),
        "env_service" => crate::env::service(&env_id, &action),
        // ---- 备份 ----
        "backup_list" => (true, crate::backup::list_json(&state.cfg)),
        "backup_dir" => crate::backup::dir_backup(&path, &state.cfg.backup.dir, keep),
        "backup_run" => crate::backup::run(&state.cfg),
        "backup_schedule" => crate::backup::schedule(&state.cfg, &cron),
        "backup_schedule_remove" => crate::backup::schedule_remove(),
        "backup_cloud" => crate::backup::cloud_upload(&file),
        // ---- 安全 ----
        "security_bans" => (true, crate::security::bans_json()),
        "security_hardening" => (true, crate::security::hardening_status()),
        "security_waf_status" => (true, crate::extra::waf_status_json(&state.cfg)),
        "security_ban" => crate::security::ban_ip(&ip),
        "security_unban" => crate::security::unban_ip(&ip),
        "security_brute" => (true, crate::security::brute_scan(threshold)),
        "security_waf_enable" => crate::security::waf_apply(rps, burst),
        "security_waf_disable" => crate::security::waf_disable(),
        "security_harden" => crate::security::harden_ssh(no_root_pass, no_password),
        "security_unharden" => crate::security::unharden_ssh(),
        // ---- IotaPanel 兼容插件 ----
        "iota_list" => (true, state.iota.list_json()),
        "iota_status" => (true, state.iota.status_json(&fname)),
        "iota_log" => (true, state.iota.log_tail_json(&fname, n)),
        "iota_start" => match state.iota.start(&fname) {
            Ok((_, p)) => (true, format!("插件 {} 已启动（端口 {}）", fname, p)),
            Err(e) => (false, e),
        },
        "iota_stop" => state.iota.stop(&fname),
        "iota_restart" => state.iota.restart(&fname),
        "iota_uninstall" => state.iota.uninstall(&fname),
        "iota_keepalive" => state.iota.set_keepalive(&fname, enable),
        "iota_install_url" => state.iota.install_url(&url, &sha256),
        // ---- HTTPS 反向代理网关 ----
        "proxy_list" => (true, state.proxies.list_json()),
        "proxy_add" => (true, state.proxies.add_json(&prefix, &target)),
        "proxy_del" => (true, state.proxies.del_json(&prefix)),
        "system_restart" => {
            let (ok, msg) = crate::system::self_restart();
            (ok, msg)
        }
        // ---- 插件商店 / KV / 启停 ----
        "plugin_store" => (true, state.plugins.store_list_json(&state.cfg)),
        "plugin_store_install" => state.plugins.store_install(&env_id, &state.cfg),
        "plugin_kv" => (true, state.plugins.kv_list_json()),
        "plugin_enable" => state.plugins.set_enabled(&fname, true),
        "plugin_disable" => state.plugins.set_enabled(&fname, false),
        // ---- 监控 / 资源排行 ----
        "resource_top" => (true, crate::extra::resources_top_json(n)),
        "monitor_snapshot" => (true, crate::monitor::monitor_json(n)),
        "shop_list" => (true, state.shop.list_json(&state.cfg)),
        "log_follow" => (true, crate::extra::log_follow_json(&file, pos)),
        "disk_usage" => (true, crate::extra::disk_usage_json()),
        "file_mkdir" => crate::extra::mkdir(&path),
        "file_rename" => crate::extra::rename(&src, &dst),
        "docker_containers" => (true, crate::extra::docker_containers_json()),
        "docker_action" => crate::extra::docker_action(&fname, &action),
        // ===== ops 运维工具箱 =====
        // 网络诊断
        "ping" => (true, crate::ops::ping(&host, count)),
        "tcp_ping" => (true, crate::ops::tcp_ping(&host, tport, count)),
        "dns_lookup" => (true, crate::ops::dns_lookup(&host)),
        "http_head" => (true, crate::ops::http_head(&url)),
        "listener_ports" => (true, crate::ops::listener_ports()),
        "port_check" => (true, crate::ops::port_check(tport)),
        "reverse_dns" => (true, crate::ops::reverse_dns(&ip)),
        // 系统纵深
        "cpu_info" => (true, crate::ops::cpu()),
        "cpu_usage" => (true, crate::ops::cpu_usage()),
        "mem_info" => (true, crate::ops::mem_info()),
        "swap_info" => (true, crate::ops::swap_info()),
        "loadavg" => (true, crate::ops::loadavg()),
        "net_io" => (true, crate::ops::net_io()),
        "disk_inodes" => (true, crate::ops::disk_inodes()),
        "os_release" => (true, crate::ops::os_release()),
        "kernel_info" => (true, crate::ops::kernel_info()),
        // 文件系统深度
        "ls_long" => (true, crate::ops::ls_long(&path)),
        "dir_size" => (true, crate::ops::dir_size(&path)),
        "file_count" => (true, crate::ops::file_count(&path)),
        "file_search" => (true, crate::ops::file_search(&path, &pattern)),
        "file_chmod" => crate::ops::file_chmod(&path, &mode),
        "zip_archive" => crate::ops::zip_archive(&src, &dst),
        "zip_extract" => crate::ops::zip_extract(&file, &dest),
        "file_head" => (true, crate::ops::file_head(&path, n)),
        "file_size" => (true, crate::ops::file_size(&path)),
        // 进程
        "process_by_name" => (true, crate::ops::process_by_name(&fname)),
        "process_detail" => (true, crate::ops::process_detail(tpid)),
        // 服务 / systemd
        "systemd_units" => (true, crate::ops::systemd_units()),
        "systemd_action" => crate::ops::systemd_action(&unit, &action),
        // 软件包(apt)
        "apt_update" => crate::ops::apt_update(),
        "apt_upgrade" => crate::ops::apt_upgrade(),
        "apt_install" => crate::ops::apt_install(&pkg),
        "apt_remove" => crate::ops::apt_remove(&pkg),
        "apt_list_installed" => (true, crate::ops::apt_list_installed()),
        "pkg_installed" => (true, crate::ops::pkg_installed(&pkg)),
        // 计划任务
        "cron_list" => (true, crate::ops::cron_list()),
        "cron_add" => crate::ops::cron_add(&schedule, &command),
        "cron_remove" => crate::ops::cron_remove(&keyword),
        "cron_system" => (true, crate::ops::cron_system()),
        // 运行时版本
        "php_version" => (true, crate::ops::php_version()),
        "node_version" => (true, crate::ops::node_version()),
        "go_version" => (true, crate::ops::go_version()),
        "python_version" => (true, crate::ops::python_version()),
        "mysql_version" => (true, crate::ops::mysql_version()),
        "php_fpm_sockets" => (true, crate::ops::php_fpm_sockets()),
        // 数据库深化
        "db_sizes" => crate::ops::db_sizes(&state.cfg.database),
        "mysql_status" => crate::ops::mysql_status(&state.cfg.database),
        "mysql_ping" => (true, crate::ops::mysql_ping(&state.cfg.database)),
        // SSL 深化
        "cert_view" => crate::ops::cert_view(&fname, &state.cfg.certs),
        "cert_expiry" => (true, crate::ops::cert_expiry(&fname, &state.cfg.certs)),
        // Docker 深化
        "docker_images" => (true, crate::ops::docker_images()),
        "docker_stats" => (true, crate::ops::docker_stats()),
        "docker_prune" => crate::ops::docker_prune(),
        "docker_info" => (true, crate::ops::docker_info_json()),
        // 日志
        "dmesg_tail" => (true, crate::ops::dmesg_tail(n)),
        "journal_tail" => (true, crate::ops::journal_tail(&unit, n)),
        "nginx_error_tail" => (true, crate::ops::nginx_error_tail(n)),
        "nginx_access_tail" => (true, crate::ops::nginx_access_tail(n)),
        "mysql_error_tail" => (true, crate::ops::mysql_error_tail(n)),
        "auth_log_tail" => (true, crate::ops::auth_log_tail(n)),
        // 用户 / 杂项
        "users_list" => (true, crate::ops::users_list()),
        "whoami" => (true, crate::ops::whoami()),
        "random_password" => (true, crate::ops::random_password(len)),
        "sha256" => (true, crate::ops::sha256(&text)),
        "base64_encode" => (true, crate::ops::base64_encode(&text)),
        "base64_decode" => (true, crate::ops::base64_decode(&enc)),
        "uuid" => (true, crate::ops::uuid()),
        "panel_about" => (true, crate::ops::panel_about()),
        // ops 九、安全 / 网络 / 系统实用
        "firewall_rules" => (true, crate::ops::firewall_rules()),
        "net_interfaces" => (true, crate::ops::net_interfaces()),
        "route_table" => (true, crate::ops::route_table()),
        "public_ip" => (true, crate::ops::public_ip()),
        "uptime" => (true, crate::ops::uptime()),
        "hostname" => (true, crate::ops::hostname()),
        "who_online" => (true, crate::ops::who_online()),
        "last_logins" => (true, crate::ops::last_logins()),
        "cpu_per_core" => (true, crate::ops::cpu_per_core()),
        "zombie_count" => (true, crate::ops::zombie_count()),
        "file_tail" => (true, crate::ops::file_tail(&fname, tlines)),
        "time_now" => (true, crate::ops::time_now()),
        "random_int" => (true, crate::ops::random_int(min, max)),
        "read_env" => (true, crate::ops::read_env(&fname)),
        // ===== ops2 第二批（补齐派发）=====
        "arp_table" => (true, crate::ops2::arp_table()),
        "dns_mx" => (true, crate::ops2::dns_mx(&host)),
        "dns_ns" => (true, crate::ops2::dns_ns(&host)),
        "dns_txt" => (true, crate::ops2::dns_txt(&host)),
        "traceroute_run" => (true, crate::ops2::traceroute_run(&host)),
        "tcp_state_summary" => (true, crate::ops2::tcp_state_summary()),
        "established_count" => (true, crate::ops2::established_count()),
        "listen_ipv6" => (true, crate::ops2::listen_ipv6()),
        "gateway_ip" => (true, crate::ops2::gateway_ip()),
        "mac_by_ip" => (true, crate::ops2::mac_by_ip(&ip)),
        "os_version_id" => (true, crate::ops2::os_version_id()),
        "arch" => (true, crate::ops2::arch()),
        "core_count" => (true, crate::ops2::core_count()),
        "context_switches" => (true, crate::ops2::context_switches()),
        "processes_count" => (true, crate::ops2::processes_count()),
        "processes_blocked" => (true, crate::ops2::processes_blocked()),
        "processes_running" => (true, crate::ops2::processes_running()),
        "boot_time" => (true, crate::ops2::boot_time()),
        "cache_mem" => (true, crate::ops2::cache_mem()),
        "mem_available" => (true, crate::ops2::mem_available()),
        "file_stat" => (true, crate::ops2::file_stat(&path)),
        "file_copy" => crate::ops2::file_copy(&src, &dst),
        "file_delete" => crate::ops2::file_delete(&path),
        "file_touch" => crate::ops2::file_touch(&path),
        "file_append" => crate::ops2::file_append(&path, &content),
        "file_find" => (true, crate::ops2::file_find(&path, &fname)),
        "file_md5" => (true, crate::ops2::file_md5(&path)),
        "file_wc" => (true, crate::ops2::file_wc(&path)),
        "du_root" => (true, crate::ops2::du_root(&path)),
        "ln_symlink" => crate::ops2::ln_symlink(&target, &link),
        "process_tree" => (true, crate::ops2::process_tree()),
        "process_threads_of" => (true, crate::ops2::process_threads_of(tpid)),
        "process_children_of" => (true, crate::ops2::process_children_of(tpid)),
        "process_cwd" => (true, crate::ops2::process_cwd(tpid)),
        "process_cmdline" => (true, crate::ops2::process_cmdline(tpid)),
        "process_top_cpu" => (true, crate::ops2::process_top_cpu(n as u32)),
        "process_top_mem" => (true, crate::ops2::process_top_mem(n as u32)),
        "process_state_count" => (true, crate::ops2::process_state_count()),
        "kill_process_by_name" => crate::ops2::kill_process_by_name(&fname),
        "nice_set" => crate::ops2::nice_set(tpid, nice),
        "apt_search" => (true, crate::ops2::apt_search(&keyword)),
        "apt_pkg_info" => (true, crate::ops2::apt_pkg_info(&pkg)),
        "apt_depends" => (true, crate::ops2::apt_depends(&pkg)),
        "dpkg_count" => (true, crate::ops2::dpkg_count()),
        "pip_version" => (true, crate::ops2::pip_version()),
        "nginx_version" => (true, crate::ops2::nginx_version()),
        "redis_version" => (true, crate::ops2::redis_version()),
        "docker_version" => (true, crate::ops2::docker_version()),
        "git_version" => (true, crate::ops2::git_version()),
        "curl_version" => (true, crate::ops2::curl_version()),
        "systemd_failed" => (true, crate::ops2::systemd_failed()),
        "systemd_enabled" => (true, crate::ops2::systemd_enabled()),
        "systemd_timers" => (true, crate::ops2::systemd_timers()),
        "port_owner" => (true, crate::ops2::port_owner(tport)),
        "cron_full" => (true, crate::ops2::cron_full()),
        "at_jobs" => (true, crate::ops2::at_jobs()),
        "wanted_units" => (true, crate::ops2::wanted_units()),
        "login_sessions" => (true, crate::ops2::login_sessions()),
        "journal_size" => (true, crate::ops2::journal_size()),
        "tmp_count" => (true, crate::ops2::tmp_count()),
        "uid0_users" => (true, crate::ops2::uid0_users()),
        "sudoers_users" => (true, crate::ops2::sudoers_users()),
        "ssh_keys_present" => (true, crate::ops2::ssh_keys_present()),
        "ssh_param" => (true, crate::ops2::ssh_param(&param)),
        "open_ports_summary" => (true, crate::ops2::open_ports_summary()),
        "pending_upgrades" => (true, crate::ops2::pending_upgrades()),
        "failed_auths" => (true, crate::ops2::failed_auths(n as u32)),
        "listening_uid_owners" => (true, crate::ops2::listening_uid_owners()),
        "mounts_with_exec" => (true, crate::ops2::mounts_with_exec()),
        "sensitive_perms" => (true, crate::ops2::sensitive_perms()),
        "md5_digest" => (true, crate::ops2::md5_digest(&text)),
        "sha1_digest" => (true, crate::ops2::sha1_digest(&text)),
        "cksum_text" => (true, crate::ops2::cksum_text(&text)),
        "url_encode" => (true, crate::ops2::url_encode(&text)),
        "url_decode" => (true, crate::ops2::url_decode(&text)),
        "hex_encode" => (true, crate::ops2::hex_encode(&text)),
        "hex_decode" => (true, crate::ops2::hex_decode(&text)),
        "base32_encode" => (true, crate::ops2::base32_encode(&text)),
        "upper_case" => (true, crate::ops2::upper_case(&text)),
        "lower_case" => (true, crate::ops2::lower_case(&text)),
        "wc_lines" => (true, crate::ops2::wc_lines(&path)),
        "wc_words" => (true, crate::ops2::wc_words(&path)),
        "grep_count" => (true, crate::ops2::grep_count(&path, &pattern)),
        "grep_lines" => (true, crate::ops2::grep_lines(&path, &pattern)),
        "sort_numeric" => (true, crate::ops2::sort_numeric(&text)),
        "unique_lines" => (true, crate::ops2::unique_lines(&text)),
        "cut_field" => (true, crate::ops2::cut_field(&text, &delim, field)),
        "tr_replace" => (true, crate::ops2::tr_replace(&text, &from, &to)),
        "append_line_once" => crate::ops2::append_line_once(&path, &content),
        "csv_fields" => (true, crate::ops2::csv_fields(&text, &sep)),
        "epoch_to_time" => (true, crate::ops2::epoch_to_time(epoch)),
        "timezone_offset" => (true, crate::ops2::timezone_offset()),
        "random_token" => (true, crate::ops2::random_token(len)),
        "rand_bool" => (true, crate::ops2::rand_bool()),
        "valid_ip" => (true, crate::ops2::valid_ip(&ip)),
        "valid_domain" => (true, crate::ops2::valid_domain(&host)),
        "default_route_via" => (true, crate::ops2::default_route_via()),
        "dns_servers" => (true, crate::ops2::dns_servers()),
        "swap_usage" => (true, crate::ops2::swap_usage()),
        "disk_io_simple" => (true, crate::ops2::disk_io_simple()),
        // ===== ops3 第三批 100 个 =====
        "iface_list" => (true, crate::ops3::iface_list()),
        "iface_speed" => (true, crate::ops3::iface_speed(&iface)),
        "iface_duplex" => (true, crate::ops3::iface_duplex(&iface)),
        "iface_mac" => (true, crate::ops3::iface_mac(&iface)),
        "iface_up" => (true, crate::ops3::iface_up(&iface)),
        "traffic_since_boot" => (true, crate::ops3::traffic_since_boot()),
        "udp_listen" => (true, crate::ops3::udp_listen()),
        "unix_sockets" => (true, crate::ops3::unix_sockets()),
        "ip_v6_addr" => (true, crate::ops3::ip_v6_addr()),
        "ping_loss" => (true, crate::ops3::ping_loss(&host)),
        "kernel_release" => (true, crate::ops3::kernel_release()),
        "kernel_version" => (true, crate::ops3::kernel_version()),
        "hostname_full" => (true, crate::ops3::hostname_full()),
        "kernel_config" => (true, crate::ops3::kernel_config(&param)),
        "vm_params" => (true, crate::ops3::vm_params()),
        "fs_params" => (true, crate::ops3::fs_params()),
        "net_params" => (true, crate::ops3::net_params()),
        "entropy_avail" => (true, crate::ops3::entropy_avail()),
        "allowed_ports" => (true, crate::ops3::allowed_ports()),
        "mem_zones" => (true, crate::ops3::mem_zones()),
        "mount_list" => (true, crate::ops3::mount_list()),
        "mount_by_point" => (true, crate::ops3::mount_by_point(&point)),
        "disk_uuid" => (true, crate::ops3::disk_uuid()),
        "disk_model" => (true, crate::ops3::disk_model()),
        "disk_readonly" => (true, crate::ops3::disk_readonly()),
        "inode_usage" => (true, crate::ops3::inode_usage(&path)),
        "fs_type" => (true, crate::ops3::fs_type(&path)),
        "block_devices" => (true, crate::ops3::block_devices()),
        "sector_size" => (true, crate::ops3::sector_size()),
        "swap_devices" => (true, crate::ops3::swap_devices()),
        "user_home" => (true, crate::ops3::user_home(&usr)),
        "user_shell" => (true, crate::ops3::user_shell(&usr)),
        "user_groups" => (true, crate::ops3::user_groups(&usr)),
        "group_members" => (true, crate::ops3::group_members(&grp)),
        "user_last_login" => (true, crate::ops3::user_last_login(&usr)),
        "lock_users" => (true, crate::ops3::lock_users()),
        "logins_total" => (true, crate::ops3::logins_total()),
        "nologin_users" => (true, crate::ops3::nologin_users()),
        "file_owner" => (true, crate::ops3::file_owner(&path)),
        "effective_uid" => (true, crate::ops3::effective_uid()),
        "selinux_status" => (true, crate::ops3::selinux_status()),
        "apparmor_status" => (true, crate::ops3::apparmor_status()),
        "world_writable" => (true, crate::ops3::world_writable()),
        "suid_bins" => (true, crate::ops3::suid_bins()),
        "socket_perms" => (true, crate::ops3::socket_perms()),
        "ip_forward" => (true, crate::ops3::ip_forward()),
        "firewall_active" => (true, crate::ops3::firewall_active()),
        "listen_low_ports" => (true, crate::ops3::listen_low_ports()),
        "umask_current" => (true, crate::ops3::umask_current()),
        "core_pattern" => (true, crate::ops3::core_pattern()),
        "char_count" => (true, crate::ops3::char_count(&text)),
        "str_reverse" => (true, crate::ops3::str_reverse(&text)),
        "word_count" => (true, crate::ops3::word_count(&text)),
        "is_empty" => (true, crate::ops3::is_empty(&text)),
        "has_digits" => (true, crate::ops3::has_digits(&text)),
        "has_upper" => (true, crate::ops3::has_upper(&text)),
        "dashed_line" => (true, crate::ops3::dashed_line()),
        "repeat_str" => (true, crate::ops3::repeat_str(&text, n as u32)),
        "title_case" => (true, crate::ops3::title_case(&text)),
        "swap_case" => (true, crate::ops3::swap_case(&text)),
        "sum_list" => (true, crate::ops3::sum_list(&text)),
        "avg_list" => (true, crate::ops3::avg_list(&text)),
        "min_max" => (true, crate::ops3::min_max(&text)),
        "median" => (true, crate::ops3::median(&text)),
        "is_prime" => (true, crate::ops3::is_prime(n as u64)),
        "factorial" => (true, crate::ops3::factorial(n as u32)),
        "gcd" => (true, crate::ops3::gcd(a, b)),
        "lcm" => (true, crate::ops3::lcm(a, b)),
        "power" => (true, crate::ops3::power(base, exp)),
        "percentage" => (true, crate::ops3::percentage(&text)),
        "uptime_seconds" => (true, crate::ops3::uptime_seconds()),
        "utc_now" => (true, crate::ops3::utc_now()),
        "iso_date" => (true, crate::ops3::iso_date()),
        "weekday" => (true, crate::ops3::weekday()),
        "quarter" => (true, crate::ops3::quarter()),
        "seconds_until" => (true, crate::ops3::seconds_until(end_unix)),
        "calendar_seed" => (true, crate::ops3::calendar_seed()),
        "is_leap_year" => (true, crate::ops3::is_leap_year(year)),
        "day_count_month" => (true, crate::ops3::day_count_month()),
        "time_signed_bin" => (true, crate::ops3::time_signed_bin()),
        "pid_count_all" => (true, crate::ops3::pid_count_all()),
        "process_start" => (true, crate::ops3::process_start(tpid)),
        "process_rss" => (true, crate::ops3::process_rss(tpid)),
        "process_vsz" => (true, crate::ops3::process_vsz(tpid)),
        "process_state" => (true, crate::ops3::process_state(tpid)),
        "open_files" => (true, crate::ops3::open_files(tpid)),
        "io_by_pid" => (true, crate::ops3::io_by_pid(tpid)),
        "longest_cmdline" => (true, crate::ops3::longest_cmdline()),
        "thread_total" => (true, crate::ops3::thread_total()),
        "dir_nlink" => (true, crate::ops3::dir_nlink(&path)),
        "tar_list" => (true, crate::ops3::tar_list(&file)),
        "gz_info" => (true, crate::ops3::gz_info(&file)),
        "sha256sum_file" => (true, crate::ops3::sha256sum_file(&file)),
        "env_all" => (true, crate::ops3::env_all()),
        "echo_args" => (true, crate::ops3::echo_args(&text)),
        "len_bytes" => (true, crate::ops3::len_bytes(&text)),
        "is_numeric" => (true, crate::ops3::is_numeric(&text)),
        "to_int" => (true, crate::ops3::to_int(&text)),
        "byte_units" => (true, crate::ops3::byte_units(bytes)),
        "is_systemd" => (true, crate::ops3::is_systemd()),
        // ==== ops4 数学与单位换算 ====
        "is_whole" => (true, crate::ops4::is_whole(&text)),
        "is_positive" => (true, crate::ops4::is_positive(&text)),
        "is_negative" => (true, crate::ops4::is_negative(&text)),
        "is_even" => (true, crate::ops4::is_even(&text)),
        "is_odd" => (true, crate::ops4::is_odd(&text)),
        "is_integer_range" => (true, crate::ops4::is_integer_range(&text)),
        "floor_num" => (true, crate::ops4::floor_num(&text)),
        "ceil_num" => (true, crate::ops4::ceil_num(&text)),
        "trunc_num" => (true, crate::ops4::trunc_num(&text)),
        "round_num" => (true, crate::ops4::round_num(&text)),
        "abs_num" => (true, crate::ops4::abs_num(&text)),
        "sign_num" => (true, crate::ops4::sign_num(&text)),
        "negate_num" => (true, crate::ops4::negate_num(&text)),
        "add_three" => (true, crate::ops4::add_three(&sa, &sb, &c)),
        "mul_three" => (true, crate::ops4::mul_three(&sa, &sb, &c)),
        "sub2" => (true, crate::ops4::sub2(&sa, &sb)),
        "div2" => (true, crate::ops4::div2(&sa, &sb)),
        "rem2" => (true, crate::ops4::rem2(&sa, &sb)),
        "avg2" => (true, crate::ops4::avg2(&sa, &sb)),
        "avg3" => (true, crate::ops4::avg3(&sa, &sb, &c)),
        "square_num" => (true, crate::ops4::square_num(&text)),
        "cube_num" => (true, crate::ops4::cube_num(&text)),
        "sqrt_num" => (true, crate::ops4::sqrt_num(&text)),
        "inv_num" => (true, crate::ops4::inv_num(&text)),
        "pow2" => (true, crate::ops4::pow2(&text)),
        "pow10x" => (true, crate::ops4::pow10x(&text)),
        "log10_num" => (true, crate::ops4::log10_num(&text)),
        "log2_num" => (true, crate::ops4::log2_num(&text)),
        "ln_num" => (true, crate::ops4::ln_num(&text)),
        "exp_num" => (true, crate::ops4::exp_num(&text)),
        "sin_deg" => (true, crate::ops4::sin_deg(&text)),
        "cos_deg" => (true, crate::ops4::cos_deg(&text)),
        "tan_deg" => (true, crate::ops4::tan_deg(&text)),
        "rad2deg" => (true, crate::ops4::rad2deg(&text)),
        "deg2rad" => (true, crate::ops4::deg2rad(&text)),
        "fib_num" => (true, crate::ops4::fib_num(&text)),
        "triangular_num" => (true, crate::ops4::triangular_num(&text)),
        "digit_sum" => (true, crate::ops4::digit_sum(&text)),
        "digit_count" => (true, crate::ops4::digit_count(&text)),
        "collatz_steps" => (true, crate::ops4::collatz_steps(&text)),
        "is_prime2" => (true, crate::ops4::is_prime2(&text)),
        "next_prime" => (true, crate::ops4::next_prime(&text)),
        "circle_area" => (true, crate::ops4::circle_area(&text)),
        "circle_circumference" => (true, crate::ops4::circle_circumference(&text)),
        "sphere_volume" => (true, crate::ops4::sphere_volume(&text)),
        "pythagoras" => (true, crate::ops4::pythagoras(&sa, &sb)),
        "rect_perimeter" => (true, crate::ops4::rect_perimeter(&sa, &sb)),
        "rect_area" => (true, crate::ops4::rect_area(&sa, &sb)),
        "c2f" => (true, crate::ops4::c2f(&text)),
        "f2c" => (true, crate::ops4::f2c(&text)),
        "c2k" => (true, crate::ops4::c2k(&text)),
        "k2c" => (true, crate::ops4::k2c(&text)),
        "bytes_to_kb" => (true, crate::ops4::bytes_to_kb(&text)),
        "bytes_to_mb" => (true, crate::ops4::bytes_to_mb(&text)),
        "bytes_to_gb" => (true, crate::ops4::bytes_to_gb(&text)),
        "gb_to_bytes" => (true, crate::ops4::gb_to_bytes(&text)),
        "mb_to_kb" => (true, crate::ops4::mb_to_kb(&text)),
        "mbps_to_mbper_s" => (true, crate::ops4::mbps_to_mbper_s(&text)),
        "km_to_miles" => (true, crate::ops4::km_to_miles(&text)),
        "miles_to_km" => (true, crate::ops4::miles_to_km(&text)),
        "cm_to_m" => (true, crate::ops4::cm_to_m(&text)),
        "m_to_km" => (true, crate::ops4::m_to_km(&text)),
        "kg_to_lb" => (true, crate::ops4::kg_to_lb(&text)),
        "hz_to_khz" => (true, crate::ops4::hz_to_khz(&text)),
        "percent_of" => (true, crate::ops4::percent_of(&sa, &sb)),
        "random_range" => (true, crate::ops4::random_range(&text)),
        "digits_of_pi" => (true, crate::ops4::digits_of_pi(&text)),
        // ==== ops5 字符串与文本处理 ====
        "str_len" => (true, crate::ops5::str_len(&text)),
        "str_char_at" => (true, crate::ops5::str_char_at(&text, &si)),
        "str_slice" => (true, crate::ops5::str_slice(&text, &si, &sj)),
        "str_first" => (true, crate::ops5::str_first(&text)),
        "str_last" => (true, crate::ops5::str_last(&text)),
        "str_first_n" => (true, crate::ops5::str_first_n(&text, &sn)),
        "str_last_n" => (true, crate::ops5::str_last_n(&text, &sn)),
        "str_upper" => (true, crate::ops5::str_upper(&text)),
        "str_lower" => (true, crate::ops5::str_lower(&text)),
        "str_title" => (true, crate::ops5::str_title(&text)),
        "str_swap_case" => (true, crate::ops5::str_swap_case(&text)),
        "str_capitalize" => (true, crate::ops5::str_capitalize(&text)),
        "str_sentence" => (true, crate::ops5::str_sentence(&text)),
        "str_trim" => (true, crate::ops5::str_trim(&text)),
        "str_trim_start" => (true, crate::ops5::str_trim_start(&text)),
        "str_trim_end" => (true, crate::ops5::str_trim_end(&text)),
        "str_ltrim_char" => (true, crate::ops5::str_ltrim_char(&text, &s_ch)),
        "str_rtrim_char" => (true, crate::ops5::str_rtrim_char(&text, &s_ch)),
        "str_trim_char" => (true, crate::ops5::str_trim_char(&text, &s_ch)),
        "str_pad_left" => (true, crate::ops5::str_pad_left(&text, &sn, &s_ch)),
        "str_pad_right" => (true, crate::ops5::str_pad_right(&text, &sn, &s_ch)),
        "str_zfill" => (true, crate::ops5::str_zfill(&text, &sn)),
        "str_center" => (true, crate::ops5::str_center(&text, &sn, &s_ch)),
        "str_truncate" => (true, crate::ops5::str_truncate(&text, &sn)),
        "str_contains" => (true, crate::ops5::str_contains(&text, &sa)),
        "str_starts_with" => (true, crate::ops5::str_starts_with(&text, &sa)),
        "str_ends_with" => (true, crate::ops5::str_ends_with(&text, &sa)),
        "str_index_of" => (true, crate::ops5::str_index_of(&text, &sa)),
        "str_last_index" => (true, crate::ops5::str_last_index(&text, &sa)),
        "str_count" => (true, crate::ops5::str_count(&text, &sa)),
        "str_replace" => (true, crate::ops5::str_replace(&text, &sa, &sb)),
        "str_remove" => (true, crate::ops5::str_remove(&text, &sa)),
        "str_remove_all" => (true, crate::ops5::str_remove_all(&text, &sa)),
        "str_delete" => (true, crate::ops5::str_delete(&text, &si, &sj)),
        "str_insert" => (true, crate::ops5::str_insert(&text, &s_at, &sb)),
        "str_repeat" => (true, crate::ops5::str_repeat(&text, &sn)),
        "str_rev" => (true, crate::ops5::str_rev(&text)),
        "str_split" => (true, crate::ops5::str_split(&text, &sa)),
        "str_join" => (true, crate::ops5::str_join(&sa, &sep, &sb)),
        "str_word_count" => (true, crate::ops5::str_word_count(&text)),
        "str_line_count" => (true, crate::ops5::str_line_count(&text)),
        "str_number_lines" => (true, crate::ops5::str_number_lines(&text)),
        "str_first_word" => (true, crate::ops5::str_first_word(&text)),
        "str_last_word" => (true, crate::ops5::str_last_word(&text)),
        "str_alpha_count" => (true, crate::ops5::str_alpha_count(&text)),
        "str_digit_count" => (true, crate::ops5::str_digit_count(&text)),
        "str_space_count" => (true, crate::ops5::str_space_count(&text)),
        "str_punct_count" => (true, crate::ops5::str_punct_count(&text)),
        "str_vowel_count" => (true, crate::ops5::str_vowel_count(&text)),
        "str_unique_chars" => (true, crate::ops5::str_unique_chars(&text)),
        "str_is_empty" => (true, crate::ops5::str_is_empty(&text)),
        "str_is_digits" => (true, crate::ops5::str_is_digits(&text)),
        "str_is_letters" => (true, crate::ops5::str_is_letters(&text)),
        "str_is_alnum" => (true, crate::ops5::str_is_alnum(&text)),
        "str_is_upper" => (true, crate::ops5::str_is_upper(&text)),
        "str_is_lower" => (true, crate::ops5::str_is_lower(&text)),
        "str_is_space" => (true, crate::ops5::str_is_space(&text)),
        "str_is_palindrome" => (true, crate::ops5::str_is_palindrome(&text)),
        "str_to_snake" => (true, crate::ops5::str_to_snake(&text)),
        "str_to_kebab" => (true, crate::ops5::str_to_kebab(&text)),
        "str_to_camel" => (true, crate::ops5::str_to_camel(&text)),
        "str_to_pascal" => (true, crate::ops5::str_to_pascal(&text)),
        "str_edit_distance" => (true, crate::ops5::str_edit_distance(&text, &sa)),
        "str_similarity" => (true, crate::ops5::str_similarity(&text, &sa)),
        "str_quote" => (true, crate::ops5::str_quote(&text)),
        "str_unquote" => (true, crate::ops5::str_unquote(&text)),
        "str_escape" => (true, crate::ops5::str_escape(&text)),
        "str_unescape" => (true, crate::ops5::str_unescape(&text)),
        "str_indent" => (true, crate::ops5::str_indent(&text, &sn)),
        // ==== ops6 编码/校验和/进制 ====
        "dec2bin" => (true, crate::ops6::dec2bin(&text)),
        "dec2oct" => (true, crate::ops6::dec2oct(&text)),
        "dec2hex" => (true, crate::ops6::dec2hex(&text)),
        "dec2hexu" => (true, crate::ops6::dec2hexu(&text)),
        "bin2dec" => (true, crate::ops6::bin2dec(&text)),
        "oct2dec" => (true, crate::ops6::oct2dec(&text)),
        "hex2dec" => (true, crate::ops6::hex2dec(&text)),
        "bin2oct" => (true, crate::ops6::bin2oct(&text)),
        "bin2hex" => (true, crate::ops6::bin2hex(&text)),
        "oct2bin" => (true, crate::ops6::oct2bin(&text)),
        "oct2hex" => (true, crate::ops6::oct2hex(&text)),
        "hex2bin" => (true, crate::ops6::hex2bin(&text)),
        "dec2base" => (true, crate::ops6::dec2base(&text, &base_s)),
        "base2dec" => (true, crate::ops6::base2dec(&text, &base_s)),
        "dec2roman" => (true, crate::ops6::dec2roman(&text)),
        "roman2dec" => (true, crate::ops6::roman2dec(&text)),
        "sum_bytes" => (true, crate::ops6::sum_bytes(&text)),
        "xor_checksum" => (true, crate::ops6::xor_checksum(&text)),
        "djb2_hash" => (true, crate::ops6::djb2_hash(&text)),
        "djb2_hash_hex" => (true, crate::ops6::djb2_hash_hex(&text)),
        "sdbm_hash" => (true, crate::ops6::sdbm_hash(&text)),
        "fnv1a32_hash" => (true, crate::ops6::fnv1a32_hash(&text)),
        "fnv1a64_hash" => (true, crate::ops6::fnv1a64_hash(&text)),
        "adler32_cksum" => (true, crate::ops6::adler32_cksum(&text)),
        "crc32_cksum" => (true, crate::ops6::crc32_cksum(&text)),
        "hamming_weight" => (true, crate::ops6::hamming_weight(&text)),
        "count_ones" => (true, crate::ops6::count_ones(&text)),
        "parity_bit" => (true, crate::ops6::parity_bit(&text)),
        "bit_length" => (true, crate::ops6::bit_length(&text)),
        "is_power_of_two" => (true, crate::ops6::is_power_of_two(&text)),
        "utf8_hex" => (true, crate::ops6::utf8_hex(&text)),
        "hex_decode_text" => (true, crate::ops6::hex_decode_text(&text)),
        "char_code" => (true, crate::ops6::char_code(&text)),
        "char_code_hex" => (true, crate::ops6::char_code_hex(&text)),
        "utf8_len" => (true, crate::ops6::utf8_len(&text)),
        "b64_encode_text" => (true, crate::ops6::b64_encode_text(&text)),
        "b64_decode_text" => (true, crate::ops6::b64_decode_text(&text)),
        "b64url_encode_text" => (true, crate::ops6::b64url_encode_text(&text)),
        "b64url_decode_text" => (true, crate::ops6::b64url_decode_text(&text)),
        "rot13" => (true, crate::ops6::rot13(&text)),
        "caesar_shift" => (true, crate::ops6::caesar_shift(&text, &shift)),
        "xor_cipher" => (true, crate::ops6::xor_cipher(&text, &key)),
        "swap_bytes" => (true, crate::ops6::swap_bytes(&text)),
        // ==== ops7 时间与日期 ====
        "now_unix" => (true, crate::ops7::now_unix()),
        "now_millis" => (true, crate::ops7::now_millis()),
        "now_date" => (true, crate::ops7::now_date()),
        "now_time" => (true, crate::ops7::now_time()),
        "now_datetime" => (true, crate::ops7::now_datetime()),
        "now_weekday" => (true, crate::ops7::now_weekday()),
        "now_dow" => (true, crate::ops7::now_dow()),
        "now_iso" => (true, crate::ops7::now_iso()),
        "now_uptime" => (true, crate::ops7::now_uptime()),
        "unix_to_date" => (true, crate::ops7::unix_to_date(&text)),
        "unix_to_time" => (true, crate::ops7::unix_to_time(&text)),
        "unix_to_datetime" => (true, crate::ops7::unix_to_datetime(&text)),
        "unix_to_weekday" => (true, crate::ops7::unix_to_weekday(&text)),
        "unix_year" => (true, crate::ops7::unix_year(&text)),
        "unix_month" => (true, crate::ops7::unix_month(&text)),
        "unix_day" => (true, crate::ops7::unix_day(&text)),
        "unix_hour" => (true, crate::ops7::unix_hour(&text)),
        "unix_minute" => (true, crate::ops7::unix_minute(&text)),
        "unix_second" => (true, crate::ops7::unix_second(&text)),
        "unix_doy" => (true, crate::ops7::unix_doy(&text)),
        "unix_dow" => (true, crate::ops7::unix_dow(&text)),
        "is_leap_yr" => (true, crate::ops7::is_leap_yr(&text)),
        "days_in_m" => (true, crate::ops7::days_in_m(&text)),
        "days_in_yr" => (true, crate::ops7::days_in_yr(&text)),
        "date_dow" => (true, crate::ops7::date_dow(&text)),
        "date_weekday" => (true, crate::ops7::date_weekday(&text)),
        "date_doy" => (true, crate::ops7::date_doy(&text)),
        "is_weekend" => (true, crate::ops7::is_weekend(&text)),
        "week_of_year" => (true, crate::ops7::week_of_year(&text)),
        "is_month_first" => (true, crate::ops7::is_month_first(&text)),
        "is_month_last" => (true, crate::ops7::is_month_last(&text)),
        "month_name" => (true, crate::ops7::month_name(&text)),
        "month_days_txt" => (true, crate::ops7::month_days_txt(&text)),
        "date_to_unix_stamp" => (true, crate::ops7::date_to_unix_stamp(&text)),
        "add_days" => (true, crate::ops7::add_days(&text, &sa)),
        "add_hours" => (true, crate::ops7::add_hours(&text, &sa)),
        "add_minutes" => (true, crate::ops7::add_minutes(&text, &sa)),
        "add_months" => (true, crate::ops7::add_months(&text, &sa)),
        "add_years" => (true, crate::ops7::add_years(&text, &sa)),
        "date_diff_days" => (true, crate::ops7::date_diff_days(&sa, &sb)),
        "date_diff_seconds" => (true, crate::ops7::date_diff_seconds(&sa, &sb)),
        "seconds_to_hms" => (true, crate::ops7::seconds_to_hms(&text)),
        "minutes_to_hms" => (true, crate::ops7::minutes_to_hms(&text)),
        "hours_to_days" => (true, crate::ops7::hours_to_days(&text)),
        "seconds_to_human" => (true, crate::ops7::seconds_to_human(&text)),
        "ms_to_seconds" => (true, crate::ops7::ms_to_seconds(&text)),
        "days_to_years" => (true, crate::ops7::days_to_years(&text)),
        // ==== ops8 网络/地址/端口 ====
        "ip_valid_v4" => (true, crate::ops8::ip_valid_v4(&text)),
        "ip_octets" => (true, crate::ops8::ip_octets(&text)),
        "ip_octet_at" => (true, crate::ops8::ip_octet_at(&text, &si)),
        "ip_to_int" => (true, crate::ops8::ip_to_int(&text)),
        "ip_from_int" => (true, crate::ops8::ip_from_int(&text)),
        "ip_increment" => (true, crate::ops8::ip_increment(&text)),
        "ip_decrement" => (true, crate::ops8::ip_decrement(&text)),
        "ip_is_private" => (true, crate::ops8::ip_is_private(&text)),
        "ip_is_loopback" => (true, crate::ops8::ip_is_loopback(&text)),
        "ip_is_multicast" => (true, crate::ops8::ip_is_multicast(&text)),
        "ip_is_link_local" => (true, crate::ops8::ip_is_link_local(&text)),
        "ip_class" => (true, crate::ops8::ip_class(&text)),
        "ip_reverse" => (true, crate::ops8::ip_reverse(&text)),
        "mask_from_prefix" => (true, crate::ops8::mask_from_prefix(&text)),
        "prefix_from_mask" => (true, crate::ops8::prefix_from_mask(&text)),
        "subnet_network" => (true, crate::ops8::subnet_network(&text, &sa)),
        "subnet_broadcast" => (true, crate::ops8::subnet_broadcast(&text, &sa)),
        "subnet_hosts" => (true, crate::ops8::subnet_hosts(&text, &sa)),
        "cidr_contains" => (true, crate::ops8::cidr_contains(&text, &sa)),
        "cidr_first_ip" => (true, crate::ops8::cidr_first_ip(&text)),
        "cidr_last_ip" => (true, crate::ops8::cidr_last_ip(&text)),
        "cidr_size" => (true, crate::ops8::cidr_size(&text)),
        "ipv6_valid" => (true, crate::ops8::ipv6_valid(&text)),
        "ipv6_groups" => (true, crate::ops8::ipv6_groups(&text)),
        "mac_valid" => (true, crate::ops8::mac_valid(&text)),
        "mac_colon" => (true, crate::ops8::mac_colon(&text)),
        "mac_is_unicast" => (true, crate::ops8::mac_is_unicast(&text)),
        "mac_is_multicast" => (true, crate::ops8::mac_is_multicast(&text)),
        "port_valid" => (true, crate::ops8::port_valid(&text)),
        "port_service" => (true, crate::ops8::port_service(&text)),
        "port_class" => (true, crate::ops8::port_class(&text)),
        "port_range_count" => (true, crate::ops8::port_range_count(&text)),
        "port_in_range" => (true, crate::ops8::port_in_range(&text, &sa)),
        "domain_valid" => (true, crate::ops8::domain_valid(&text)),
        "domain_labels" => (true, crate::ops8::domain_labels(&text)),
        "domain_tld" => (true, crate::ops8::domain_tld(&text)),
        "domain_has_www" => (true, crate::ops8::domain_has_www(&text)),
        "email_valid" => (true, crate::ops8::email_valid(&text)),
        "email_local" => (true, crate::ops8::email_local(&text)),
        "email_domain" => (true, crate::ops8::email_domain(&text)),
        "http_status_name" => (true, crate::ops8::http_status_name(&text)),
        "http_status_class" => (true, crate::ops8::http_status_class(&text)),
        "is_2xx" => (true, crate::ops8::is_2xx(&text)),
        "is_4xx" => (true, crate::ops8::is_4xx(&text)),
        "is_5xx" => (true, crate::ops8::is_5xx(&text)),
        "ip_compare" => (true, crate::ops8::ip_compare(&text, &sa)),
        "ip_is_broadcast" => (true, crate::ops8::ip_is_broadcast(&text)),
        "ip_is_zero" => (true, crate::ops8::ip_is_zero(&text)),
        "ip_wildcard_mask" => (true, crate::ops8::ip_wildcard_mask(&text)),
        // ==== ops9 文件/路径/权限/磁盘/进程 ====
        "path_basename" => (true, crate::ops9::path_basename(&text)),
        "path_dirname" => (true, crate::ops9::path_dirname(&text)),
        "path_ext" => (true, crate::ops9::path_ext(&text)),
        "path_ext_set" => (true, crate::ops9::path_ext_set(&text, &sa)),
        "path_stem" => (true, crate::ops9::path_stem(&text)),
        "path_join" => (true, crate::ops9::path_join(&text, &sa)),
        "path_normalize" => (true, crate::ops9::path_normalize(&text)),
        "path_clean" => (true, crate::ops9::path_clean(&text)),
        "path_is_abs" => (true, crate::ops9::path_is_abs(&text)),
        "path_is_rel" => (true, crate::ops9::path_is_rel(&text)),
        "path_depth" => (true, crate::ops9::path_depth(&text)),
        "path_split" => (true, crate::ops9::path_split(&text)),
        "path_parent_all" => (true, crate::ops9::path_parent_all(&text)),
        "path_common_prefix" => (true, crate::ops9::path_common_prefix(&text, &sa)),
        "path_is_within" => (true, crate::ops9::path_is_within(&text, &sa)),
        "path_relativize" => (true, crate::ops9::path_relativize(&text, &sa)),
        "path_ensure_slash" => (true, crate::ops9::path_ensure_slash(&text)),
        "path_trim_slash" => (true, crate::ops9::path_trim_slash(&text)),
        "path_sep_count" => (true, crate::ops9::path_sep_count(&text)),
        "path_home_expand" => (true, crate::ops9::path_home_expand(&text)),
        "path_is_root" => (true, crate::ops9::path_is_root(&text)),
        "path_regex_escape" => (true, crate::ops9::path_regex_escape(&text)),
        "path_indexed" => (true, crate::ops9::path_indexed(&text, &sa)),
        "path_tail_n" => (true, crate::ops9::path_tail_n(&text, &sa)),
        "path_has_hidden" => (true, crate::ops9::path_has_hidden(&text)),
        "path_double_dots" => (true, crate::ops9::path_double_dots(&text)),
        "perm_rwx" => (true, crate::ops9::perm_rwx(&text)),
        "perm_octal" => (true, crate::ops9::perm_octal(&text)),
        "perm_symbol_sum" => (true, crate::ops9::perm_symbol_sum(&text)),
        "perm_sticky" => (true, crate::ops9::perm_sticky(&text)),
        "perm_like" => (true, crate::ops9::perm_like(&text, &sa)),
        "perm_extended" => (true, crate::ops9::perm_extended(&text)),
        "perm_world" => (true, crate::ops9::perm_world(&text)),
        "size_in_bytes" => (true, crate::ops9::size_in_bytes(&text)),
        "size_auto" => (true, crate::ops9::size_auto(&text)),
        "size_compare" => (true, crate::ops9::size_compare(&text, &sa)),
        "size_bits_units" => (true, crate::ops9::size_bits_units(&text)),
        "size_ratio" => (true, crate::ops9::size_ratio(&text, &sa)),
        "disk_fill_percent" => (true, crate::ops9::disk_fill_percent(&text, &sa)),
        "disk_free_est" => (true, crate::ops9::disk_free_est(&text, &sa)),
        "block_count" => (true, crate::ops9::block_count(&text, &sa)),
        "text_line_count" => (true, crate::ops9::text_line_count(&text)),
        "text_bytes" => (true, crate::ops9::text_bytes(&text)),
        "text_words" => (true, crate::ops9::text_words(&text)),
        "text_nonws" => (true, crate::ops9::text_nonws(&text)),
        "text_ntabs" => (true, crate::ops9::text_ntabs(&text)),
        "text_nlines_nl" => (true, crate::ops9::text_nlines_nl(&text)),
        "text_line_len" => (true, crate::ops9::text_line_len(&text)),
        "text_contains_cn" => (true, crate::ops9::text_contains_cn(&text)),
        "text_ascii_percent" => (true, crate::ops9::text_ascii_percent(&text)),
        "text_ident_lines" => (true, crate::ops9::text_ident_lines(&text)),
        "text_blank_lines" => (true, crate::ops9::text_blank_lines(&text)),
        "text_average_wlen" => (true, crate::ops9::text_average_wlen(&text)),
        "text_synopsis" => (true, crate::ops9::text_synopsis(&text)),
        "text_longest_line" => (true, crate::ops9::text_longest_line(&text)),
        "text_tabs_to_spaces" => (true, crate::ops9::text_tabs_to_spaces(&text)),
        "proc_cpu_percent" => (true, crate::ops9::proc_cpu_percent(&text, &sa)),
        "proc_vm_human" => (true, crate::ops9::proc_vm_human(&text)),
        "load_interpret" => (true, crate::ops9::load_interpret(&text)),
        "uptime_days" => (true, crate::ops9::uptime_days(&text)),
        "proc_state_desc" => (true, crate::ops9::proc_state_desc(&text)),
        "proc_user_ratio" => (true, crate::ops9::proc_user_ratio(&text, &sa)),
        "mem_percent_free" => (true, crate::ops9::mem_percent_free(&text)),
        // ==== ops10 安全/颜色/数论/日志 ====
        "entropy_of" => (true, crate::ops10::entropy_of(&text)),
        "sec_charset_size" => (true, crate::ops10::sec_charset_size(&text)),
        "pass_class_count" => (true, crate::ops10::pass_class_count(&text)),
        "pass_has_upper" => (true, crate::ops10::pass_has_upper(&text)),
        "pass_has_lower" => (true, crate::ops10::pass_has_lower(&text)),
        "pass_has_digit" => (true, crate::ops10::pass_has_digit(&text)),
        "pass_has_special" => (true, crate::ops10::pass_has_special(&text)),
        "pass_strength" => (true, crate::ops10::pass_strength(&text)),
        "pass_estimate_bits" => (true, crate::ops10::pass_estimate_bits(&text)),
        "pass_common_weak" => (true, crate::ops10::pass_common_weak(&text)),
        "html_escape" => (true, crate::ops10::html_escape(&text)),
        "html_unescape" => (true, crate::ops10::html_unescape(&text)),
        "uri_scheme" => (true, crate::ops10::uri_scheme(&text)),
        "uri_host" => (true, crate::ops10::uri_host(&text)),
        "uri_segments" => (true, crate::ops10::uri_segments(&text)),
        "hex_to_rgb" => (true, crate::ops10::hex_to_rgb(&text)),
        "rgb_to_hex" => (true, crate::ops10::rgb_to_hex(&text)),
        "is_hex" => (true, crate::ops10::is_hex(&text)),
        "hex_brightness" => (true, crate::ops10::hex_brightness(&text)),
        "hex_is_light" => (true, crate::ops10::hex_is_light(&text)),
        "hex_complement" => (true, crate::ops10::hex_complement(&text)),
        "hex_lighten" => (true, crate::ops10::hex_lighten(&text, &sa)),
        "hex_darken" => (true, crate::ops10::hex_darken(&text, &sa)),
        "hex_contrast" => (true, crate::ops10::hex_contrast(&text, &sa)),
        "hex_blend" => (true, crate::ops10::hex_blend(&text, &sa, &sr)),
        "math_fact" => (true, crate::ops10::math_fact(&text)),
        "math_gcd" => (true, crate::ops10::math_gcd(&text, &sa)),
        "math_lcm" => (true, crate::ops10::math_lcm(&text, &sa)),
        "math_modpow" => (true, crate::ops10::math_modpow(&text, &sa, &sm)),
        "math_is_prime" => (true, crate::ops10::math_is_prime(&text)),
        "math_next_prime" => (true, crate::ops10::math_next_prime(&text)),
        "math_num_divisors" => (true, crate::ops10::math_num_divisors(&text)),
        "math_digital_root" => (true, crate::ops10::math_digital_root(&text)),
        "math_is_perfect" => (true, crate::ops10::math_is_perfect(&text)),
        "math_nthfib" => (true, crate::ops10::math_nthfib(&text)),
        "math_is_coprime" => (true, crate::ops10::math_is_coprime(&text, &sa)),
        "math_triangle_type" => (true, crate::ops10::math_triangle_type(&text, &sa, &sb)),
        "log_level" => (true, crate::ops10::log_level(&text)),
        "log_ip_count" => (true, crate::ops10::log_ip_count(&text)),
        "log_error_lines" => (true, crate::ops10::log_error_lines(&text)),
        "log_warn_lines" => (true, crate::ops10::log_warn_lines(&text)),
        "log_info_lines" => (true, crate::ops10::log_info_lines(&text)),
        "log_ts_count" => (true, crate::ops10::log_ts_count(&text)),
        "log_stacktrace_lines" => (true, crate::ops10::log_stacktrace_lines(&text)),
        "log_line_lengths" => (true, crate::ops10::log_line_lengths(&text)),
        "cfg_comment_lines" => (true, crate::ops10::cfg_comment_lines(&text)),
        "cfg_brace_balance" => (true, crate::ops10::cfg_brace_balance(&text)),
        "cfg_section_count" => (true, crate::ops10::cfg_section_count(&text)),
        "cfg_equals_count" => (true, crate::ops10::cfg_equals_count(&text)),
        "json_balanced" => (true, crate::ops10::json_balanced(&text)),
        "sec_control_chars" => (true, crate::ops10::sec_control_chars(&text)),
        // ==== ops11 统计/哈希/频次/文本距离/业务/换算 ====
        "stat_sum" => (true, crate::ops11::stat_sum(&text)),
        "stat_min" => (true, crate::ops11::stat_min(&text)),
        "stat_max" => (true, crate::ops11::stat_max(&text)),
        "stat_mean" => (true, crate::ops11::stat_mean(&text)),
        "stat_median" => (true, crate::ops11::stat_median(&text)),
        "stat_mode" => (true, crate::ops11::stat_mode(&text)),
        "stat_range" => (true, crate::ops11::stat_range(&text)),
        "stat_variance" => (true, crate::ops11::stat_variance(&text)),
        "stat_stdev" => (true, crate::ops11::stat_stdev(&text)),
        "stat_geomean" => (true, crate::ops11::stat_geomean(&text)),
        "stat_peak_to_avg" => (true, crate::ops11::stat_peak_to_avg(&text)),
        "value_size" => (true, crate::ops11::value_size(&text)),
        "hash_fnv1a" => (true, crate::ops11::hash_fnv1a(&text)),
        "hash_djb2" => (true, crate::ops11::hash_djb2(&text)),
        "hash_elf" => (true, crate::ops11::hash_elf(&text)),
        "hash_adler" => (true, crate::ops11::hash_adler(&text)),
        "freq_char_count" => (true, crate::ops11::freq_char_count(&text, &s_ch)),
        "freq_top_chars" => (true, crate::ops11::freq_top_chars(&text)),
        "letter_freq" => (true, crate::ops11::letter_freq(&text)),
        "ngram_distinct" => (true, crate::ops11::ngram_distinct(&text, &s_size)),
        "levenshtein_dist" => (true, crate::ops11::levenshtein_dist(&text, &sa)),
        "hamming_dist" => (true, crate::ops11::hamming_dist(&text, &sa)),
        "palin_check" => (true, crate::ops11::palin_check(&text)),
        "anagram_check" => (true, crate::ops11::anagram_check(&text, &sa)),
        "lcs_length" => (true, crate::ops11::lcs_length(&text, &sa)),
        "dist_manhattan" => (true, crate::ops11::dist_manhattan(&text, &sa)),
        "geo_haversine" => (true, crate::ops11::geo_haversine(&text, &sa, &sc, &s_size)),
        "percent_change" => (true, crate::ops11::percent_change(&text, &sa)),
        "compound_growth" => (true, crate::ops11::compound_growth(&text, &sa, &sc)),
        "discount_price" => (true, crate::ops11::discount_price(&text, &sa)),
        "tip_split" => (true, crate::ops11::tip_split(&text, &sa, &sc)),
        "bmi_calc" => (true, crate::ops11::bmi_calc(&text, &sa)),
        "loan_payment" => (true, crate::ops11::loan_payment(&text, &sa, &sc)),
        "kelvin_to_c" => (true, crate::ops11::kelvin_to_c(&text)),
        "c_to_kelvin" => (true, crate::ops11::c_to_kelvin(&text)),
        "kmh_to_mph" => (true, crate::ops11::kmh_to_mph(&text)),
        "mph_to_kmh" => (true, crate::ops11::mph_to_kmh(&text)),
        "days_between_dates" => (true, crate::ops11::days_between_dates(&text, &sa)),
        "day_of_year" => (true, crate::ops11::day_of_year(&text)),
        "month_days_of" => (true, crate::ops11::month_days_of(&text, &sa)),
        "weekday_of" => (true, crate::ops11::weekday_of(&text)),
        "csv_row_count" => (true, crate::ops11::csv_row_count(&text)),
        "csv_first_cols" => (true, crate::ops11::csv_first_cols(&text)),
        "delim_repeat" => (true, crate::ops11::delim_repeat(&text, &s_delim)),
        _ => plugin_or_unknown(state, name.as_str(), &args),
    };
    let result = format!(
        "{{\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}],\"isError\":{}}}",
        json::jesc(&text),
        if ok { "false" } else { "true" }
    );
    fmt_jsonrpc(id, result, false)
}

/// 从 JSON 里取字符串字段（首个出现处）。
fn str_field<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\":", key);
    let idx = body.find(&needle)?;
    let rest = body[idx + needle.len()..].trim_start();
    if rest.starts_with('"') {
        let i = rest[1..].find('"')?;
        Some(&rest[1..1 + i])
    } else {
        None
    }
}

/// 从 JSON 里取数字字段（首个出现处）。
fn num_field(body: &str, key: &str) -> Option<i64> {
    let needle = format!("\"{}\":", key);
    let idx = body.find(&needle)?;
    let rest = body[idx + needle.len()..].trim_start();
    let e = rest
        .find(|c: char| c == ',' || c == '}' || c == ' ' || c == '\n')
        .unwrap_or(rest.len());
    rest[..e].trim().parse().ok()
}

/// 抽取 `arguments:{...}` 子串（简单括号匹配）。
fn extract_arg_json(body: &str) -> String {
    let key = "\"arguments\":";
    let start = match body.find(key) {
        Some(i) => i + key.len(),
        None => return String::new(),
    };
    let rest = body[start..].trim_start();
    if !rest.starts_with('{') {
        return String::new();
    }
    let mut depth = 0i32;
    let mut in_str = false;
    for (off, b) in rest.bytes().enumerate() {
        match b {
            b'"' if !in_str => in_str = true,
            b'"' if in_str => in_str = false,
            b'{' if !in_str => depth += 1,
            b'}' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    return rest[..=off].to_string();
                }
            }
            _ => {}
        }
    }
    String::new()
}

/// 在 arguments JSON 里取字符串键值。
fn arg_str(args: &str, key: &str) -> String {
    let needle = format!("\"{}\":", key);
    let idx = match args.find(&needle) {
        Some(i) => i + needle.len(),
        None => return String::new(),
    };
    let rest = args[idx..].trim_start();
    if rest.starts_with('"') {
        let i = match rest[1..].find('"') {
            Some(j) => j,
            None => return String::new(),
        };
        rest[1..1 + i].to_string()
    } else {
        // 数字/布尔
        let e = rest
            .find(|c: char| c == ',' || c == '}')
            .unwrap_or(rest.len());
        rest[..e].trim().to_string()
    }
}

/// 插件工具对应的 MCP 工具名：`p_<插件名>_<工具id>`（不做特殊字符）。
fn plugin_tool_name(p: &crate::plugins::Plugin, tool: &str) -> String {
    format!("p_{}_{}", skill(&p.name), skill(tool))
}

/// 只保留 [A-Za-z0-9_]，其余转成 `_`。
fn skill(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

/// 插件快照（避免持有锁过久）。
fn plugins_snapshot(state: &State) -> Vec<crate::plugins::Plugin> {
    state.plugins.snapshot()
}

/// 若非内置工具，尝试按插件工具派发；找不到则返回“未知工具”。
fn plugin_or_unknown(state: &State, name: &str, args: &str) -> (bool, String) {
    let amap = parse_json_obj(args);
    for p in plugins_snapshot(state) {
        for t in &p.tools {
            if plugin_tool_name(&p, &t.id) == name {
                return state.plugins.call_tool(&p.name, &t.id, amap);
            }
        }
    }
    (false, format!("未知工具: {}", name))
}

/// 宽松解析 `{"k1":"v1","k2":123,...}` 顶层对象，返回键值对（非字符串值转文本）。
fn parse_json_obj(s: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let body = s.trim();
    let body = body.strip_prefix('{').unwrap_or(body);
    let body = body.strip_suffix('}').unwrap_or(body);
    let mut i = 0;
    let b = body.as_bytes();
    while i < b.len() {
        // 找 key 引号
        while i < b.len() && b[i] != b'"' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        i += 1;
        let ks = i;
        while i < b.len() && b[i] != b'"' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let key = body[ks..i].to_string();
        i += 1;
        // 跳过冒号与空白
        while i < b.len() && (b[i] == b':' || b[i].is_ascii_whitespace()) {
            i += 1;
        }
        // 值：字符串或裸值
        if i < b.len() && b[i] == b'"' {
            i += 1;
            let vs = i;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            out.insert(key, unescape_str(&body[vs..i.min(body.len())]));
            i += 1;
        } else {
            let vs = i;
            while i < b.len() && b[i] != b',' && b[i] != b'}' {
                i += 1;
            }
            out.insert(key, body[vs..i.min(body.len())].trim().to_string());
        }
        // 跳过逗号
        while i < b.len() && b[i] != b'"' && b[i] != b'{' {
            i += 1;
        }
    }
    out
}

fn unescape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// 列出数据库备份文件（对标 api.rs 的 db_backups_json）。
fn db_backups_text(cfg: &crate::config::Config) -> String {
    let dir = &cfg.database.backup_dir;
    let mut files: Vec<(String, u64)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().extension().map_or(false, |x| x == "gz") {
                let name = e.file_name().to_string_lossy().into_owned();
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                files.push((name, size));
            }
        }
    }
    files.sort();
    let arr = files
        .iter()
        .map(|(n, s)| format!("{{\"name\":\"{}\",\"size\":{}}}", json::jesc(n), s))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"ok\":true,\"dir\":\"{}\",\"list\":[{}]}}", json::jesc(dir), arr)
}