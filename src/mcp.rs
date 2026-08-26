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
        // 只暴露面板管理核心工具；工具性/开发向的杂项一律屏蔽。
        if !is_core_tool(name) {
            continue;
        }
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
    // 仅允许面板管理核心工具；工具性/开发向的杂项一律拒绝执行。
    if !is_core_tool(&name) {
        return fmt_jsonrpc(id, format!("{{\"content\":[{{\"type\":\"text\",\"text\":\"未知或已废弃工具: {}\"}}],\"isError\":true}}", json::jesc(&name)), false);
    }
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

    let (ok, text) = match name.as_str() {
        "system_overview" => (true, crate::system::system_json(&state.monitor)),
        "list_processes" => (true, crate::system::processes_json()),
        "list_services" => (true, crate::ctl::services_json()),
        "list_firewall" => (true, crate::firewall::rules_json()),
        "list_tasks" => (true, crate::ctl::tasks_json()),
        "service_action" => crate::ctl::service_action(&svc_name, &action),
        "firewall_add" => crate::firewall::add("allow", &port, "tcp", ""),
        "firewall_del" => crate::firewall::del_by_port(&port),
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

/// 是否为「面板管理核心工具」（MCP 白名单）。
/// 面板 MCP 只应暴露服务器管理能力；纯工具性/开发向的杂项（文本/数学/编码/
/// 换算/时间/字符串等）一律剔除。
fn is_core_tool(name: &str) -> bool {
    // 插件工具（p_ 前缀）允许。
    if name.starts_with("p_") {
        return true;
    }
    let core = [
        // ---- 系统 / 进程 / 服务 / 防火墙 / 任务 ----
        "system_overview", "system_info", "system_restart",
        "list_processes", "kill_process", "resource_top",
        "list_services", "service_action",
        "list_firewall", "firewall_add", "firewall_del",
        "list_tasks", "task_add",
        "list_conns", "kill_conn",
        "monitor_snapshot",
        // ---- 文件 / 日志 ----
        "list_files", "read_file", "write_file", "delete_path",
        "file_mkdir", "file_rename", "log_tail", "log_follow",
        "disk_usage", "disk_top",
        // ---- 网站 / nginx ----
        "list_nginx", "nginx_add", "nginx_toggle", "nginx_delete", "nginx_reload", "autostart",
        "website_list", "website_create", "website_toggle", "website_delete", "website_rewrite",
        // ---- 数据库 ----
        "db_status", "db_databases", "db_users", "db_backups",
        "db_create_db", "db_drop_db", "db_create_user", "db_drop_user", "db_grant",
        "db_backup", "db_restore", "db_reset_root",
        // ---- SSL 证书 ----
        "ssl_list", "ssl_import", "ssl_self_signed", "ssl_le_issue", "ssl_apply",
        // ---- 运行环境 ----
        "env_status", "env_install", "env_service",
        // ---- 备份 ----
        "backup_list", "backup_dir", "backup_run",
        "backup_schedule", "backup_schedule_remove", "backup_cloud",
        // ---- 安全 ----
        "security_bans", "security_hardening", "security_waf_status",
        "security_ban", "security_unban", "security_brute",
        "security_waf_enable", "security_waf_disable",
        "security_harden", "security_unharden",
        // ---- 插件 / 商店 / KV ----
        "plugin_store", "plugin_store_install", "plugin_kv",
        "plugin_enable", "plugin_disable", "shop_list",
        // ---- Docker ----
        "docker_containers", "docker_action",
        // ---- 反向代理 / 兼容插件 ----
        "proxy_list", "proxy_add", "proxy_del",
        "iota_list", "iota_status", "iota_log", "iota_start",
        "iota_stop", "iota_restart", "iota_uninstall",
        "iota_keepalive", "iota_install_url",
    ];
    core.contains(&name)
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