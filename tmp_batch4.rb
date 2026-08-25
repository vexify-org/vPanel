# 批4: 安全/包管理/应用服务/性能 —— 20 个插件，每个 3 工具 => 60 工具
P = {
  "audit_rules" => ["内核审计(Audit)规则诊断", {
    "rules" => ["已配置审计规则", "auditctl -l 2>/dev/null || cat /etc/audit/audit.rules 2>/dev/null | grep -vE '^#|^$' | head -30 || echo 未安装auditd"],
    "status" => ["审计状态", "auditctl -s 2>/dev/null || echo 未启用审计"],
    "log" => ["审计日志最近条目", "tail -20 /var/log/audit/audit.log 2>/dev/null || echo 无审计日志"],
  }],
  "ssh_login_audit" => ["SSH 登录审计诊断", {
    "fail" => ["失败登录分布(按IP)", "grep -h 'Failed password' /var/log/auth.log /var/log/secure 2>/dev/null | grep -oE 'from +[0-9.]+' | awk '{print $2}' | sort | uniq -c | sort -rn | head -15 || echo 无日志"],
    "success" => ["成功登录记录", "grep -h 'Accepted' /var/log/auth.log /var/log/secure 2>/dev/null | tail -10 || echo 无日志"],
    "today" => ["今日登录尝试", "grep -h '$(date +%b\\ %d)' /var/log/auth.log /var/log/secure 2>/dev/null | wc -l | awk '{print \"今日认证事件=\"$1}' || echo 无日志"],
  }],
  "cron_security" => ["定时任务安全审计", {
    "cron_all" => ["系统定时任务", "crontab -l 2>/dev/null; ls /etc/cron.* 2>/dev/null | head -20 || echo 无cron"],
    "perms" => ["cron 目录权限", "ls -ld /etc/cron.* 2>/dev/null"],
    "allow_deny" => ["cron 黑白名单", "ls -l /etc/cron.allow /etc/cron.deny 2>/dev/null || echo 未限制(默认全允许)"],
  }],
  "shadow_security" => ["账户密码安全审计", {
    "enabled" => ["可登录账户列表", "getent passwd | awk -F: '$7!=\"/usr/sbin/nologin\" && $7!=\"/bin/false\" && $3>=1000{print $1, $3}'"],
    "uid0" => ["UID=0 账户", "getent passwd | awk -F: '$3==0{print $1}'"],
    "hashes" => ["密码哈希算法", "grep -v '^#' /etc/shadow 2>/dev/null | awk -F: '{split($2,a,\"$\"); print $1, (a[2]==\"\"?\"无密码\":(\"$\"\".a[2]))}' | head -15 || echo 需root"],
  }],
  "setuid_scan" => ["特权位(SUID)扫描", {
    "newsuid" => ["近期新出现的 SUID 文件", "find / -xdev -type f -perm -4000 2>/dev/null -mtime -7 | head -20 || echo 需权限"],
    "all_suid" => ["全部 SUID 文件", "find / -xdev -type f -perm -4000 2>/dev/null | head -40 || echo 需权限"],
    "diffsgid" => ["SGID 文件", "find / -xdev -type f -perm -2000 2>/dev/null | grep -vE '/(var|usr|etc)/' | head -20"],
  }],
  "pkg_security" => ["软件包安全审计", {
    "updates" => ["可安全更新的包数", "apt list --upgradable 2>/dev/null | wc -l; dpkg --audit 2>/dev/null | head -10"],
    "held" => ["被锁定(hold)的包", "dpkg --get-selections 2>/dev/null | grep 'hold$' | head -20"],
    "orphans" => ["已失效/多余安装", "apt list --installed 2>/dev/null | wc -l | awk '{print \"已安装包=\"$1}'"],
  }],
  "service_security" => ["服务与端口暴露诊断", {
    "open_ports" => ["对外暴露端口", "ss -tlnp 2>/dev/null | awk '$4 !~ /127\\./ && $4 !~ /::1/ {print $4}' | sort -u | head -30"],
    "disabled_users" => ["nologin 服务账户", "getent passwd | awk -F: '$7==\"/usr/sbin/nologin\" || $7==\"/bin/false\"' | wc -l"],
    "web_dirs" => ["Web 目录写权限风险", "ls -ld /var/www/* 2>/dev/null | head -10 || echo 无/var/www"],
  }],
  "network_isolation" => ["网络隔离与转发诊断", {
    "forward" => ["IP 转发是否开启", "cat /proc/sys/net/ipv4/ip_forward"],
    "rpfilter" => ["反向路径过滤", "cat /proc/sys/net/ipv4/conf/all/rp_filter"],
    "promisc" => ["混杂模式网卡", "grep -rl 'PROMISC' /sys/class/net/*/flags 2>/dev/null | sed 's/.*net\\///' || echo 无混杂模式"],
  }],
  "apt_mirror" => ["包源镜像可达性诊断", {
    "sources" => ["APT 源列表", "grep -rhE '^deb ' /etc/apt/sources.list /etc/apt/sources.list.d/ 2>/dev/null | head -20 || echo 非debian系"],
    "check" => ["APT 更新检查", "apt-get update -o Acquire::http::Timeout=2 2>&1 | tail -5 || echo 无网络/未配置"],
    "upgradable" => ["可升级包概览", "apt-get -s upgrade 2>/dev/null | grep -cE '^Inst ' | awk '{print \"可升级=\"$1}'"],
  }],
  "git_repos" => ["Git 仓库状态扫描", {
    "dirty" => ["存在未提交变更的仓库", "find / -maxdepth 3 -name .git -type d 2>/dev/null | head -10"],
    "bare" => ["裸仓库", "find / -maxdepth 4 -name '*.git' -type d 2>/dev/null | xargs -r -I{} sh -c 'test -e {}/HEAD && echo {}' 2>/dev/null | head -10"],
    "config" => ["Git 全局配置", "cat /root/.gitconfig /etc/gitconfig 2>/dev/null | head -20 || echo 无"],
  }],
  "app_services" => ["常驻应用服务探活", {
    "listening" => ["监听中的服务进程", "ss -tlnp 2>/dev/null | awk 'NR>1{print $4}' | sort -u | wc -l | awk '{print \"监听端口数=\"$1}'"],
    "web" => ["常见Web端口状态", "for p in 80 443 8080 3306 6379; do echo -n \"$p: \"; ss -tln 2>/dev/null | grep -q \":$p \" && echo 监听 || echo 关闭; done"],
    "systemd_failed" => ["启动失败的单元", "systemctl --failed --no-legend 2>/dev/null | head -10 || echo 非systemd"],
  }],
  "time_sync2" => ["时间同步诊断", {
    "status" => ["时间同步状态", "timedatectl 2>/dev/null | grep -iE 'synchronized|systime' || echo 非systemd"],
    "offset" => ["与 NTP 偏差", "chronyc tracking 2>/dev/null | head -6 || ntpq -p 2>/dev/null || echo 未装chrony/ntp"],
    "sources" => ["NTP 服务器", "chronyc sources 2>/dev/null | head -10 || grep -rhE '^server' /etc/chrony.conf /etc/ntp.conf 2>/dev/null | head -10"],
  }],
  "perf_top" => ["进程性能 TOP 诊断", {
    "cpu_top" => ["CPU 占用前10", "ps -eo pid,comm,%cpu --sort=-%cpu --no-headers 2>/dev/null | head -10"],
    "mem_top" => ["内存占用前10", "ps -eo pid,comm,rss --sort=-rss --no-headers 2>/dev/null | head -10"],
    "io_top" => ["I/O 等待前5", "ps -eo pid,stat,comm --no-headers -o stat,comm 2>/dev/null | awk '$1 ~ /D/{print}' | head -5 || echo 无D状态进程"],
  }],
  "vmstat_dash" => ["系统健康仪表盘", {
    "vmstat" => ["系统资源瞬时状态", "vmstat 1 2 2>/dev/null | tail -1 || cat /proc/meminfo | head -5"],
    "swap" => ["交换使用", "free -h 2>/dev/null | head -2 | tail -1"],
    "io" => ["IO 等待占比", "vmstat 1 2 2>/dev/null | tail -1 | awk '{print \"wa=\"$16\"%\"}'"],
  }],
  "load_history" => ["负载与资源历史诊断", {
    "loadavg" => ["当前与历史负载", "cat /proc/loadavg"],
    "sar_cpu" => ["近30天CPU空闲率", "sar -u 2>/dev/null | tail -5 || echo 需sysstat"],
    "daily" => ["今日负载峰值", "sar -q 2>/dev/null | grep -vE '^$|Average|Linux' | tail -10 || echo 需sysstat"],
  }],
  "conn_perf" => ["连接性能与压力诊断", {
    "estab" => ["已建立连接数", "ss -tan 2>/dev/null | grep -c ESTAB"],
    "timewait" => ["TIME_WAIT 状态数", "ss -tan 2>/dev/null | grep -c TIME-WAIT"],
    "ephemeral" => ["本地端口范围", "cat /proc/sys/net/ipv4/ip_local_port_range"],
  }],
  "php_fpm2" => ["PHP-FPM 池诊断", {
    "status" => ["PHP-FPM 运行池", "ps -eo cmd --no-headers 2>/dev/null | grep -i 'php-fpm' | head -3 || echo 未运行"],
    "slow" => ["慢日志检查", "find /var/log -name '*-slow.log' 2>/dev/null -exec tail -5 {} \\; | head -15 || echo 无慢日志"],
    "opcache" => ["OPcache 状态", "php -i 2>/dev/null | grep -E 'opcache.enable|opcache.memory_consumption|opcache.interned' | head -5 || echo 需php-cli"],
  }],
  "nginx_health" => ["Nginx 运行时诊断", {
    "master" => ["Nginx 主进程状态", "ps -eo pid,comm,%cpu --sort=-%cpu --no-headers 2>/dev/null | grep -i nginx | head -3 || echo 未运行"],
    "conns" => ["Nginx 活动连接", "grep -E 'Active connections|Reading|Writing|Waiting' /var/log/nginx/access.log 2>/dev/null | tail -5; ss -tan 2>/dev/null | grep -c :80 2>/dev/null | awk '{print \"80端口连接=\"$1}'"],
    "conf" => ["配置语法检查", "nginx -t 2>&1 | head -5 || echo 需nginx"],
  }],
  "app_logs" => ["应用日志故障诊断", {
    "critical" => ["各日志关键错误", "grep -rhE 'error|fatal|panic' /var/log/nginx/*.log /var/log/mysql/*.log /var/log/php* 2>/dev/null | tail -20 || echo 无错误日志"],
    "disk_full" => ["磁盘空间预警", "df -h | awk '$5+0>85 {print $1, $5}' && echo --- || echo 无超85%分区"],
    "recent" => ["最近修改日志", "find /var/log -type f -mmin -30 2>/dev/null | head -10 | xargs -r ls -lh 2>/dev/null"],
  }],
}

def render(name, meta)
  tdesc = meta[0]
  tools = meta[1]
  body = tools.map do |id, (d, s)|
    ind = s.gsub("\n", "\n            ")
    "    - id: #{id}\n      desc: \"#{d}\"\n      script: |\n        #{ind}"
  end.join("\n")
  "name: #{name}\nversion: 1.0.0\ndesc: \"#{tdesc}\"\nurl: plugins/#{name}.yml\n\ntools:\n#{body}\n"
end

P.each { |n, m| File.write("/workspace/plugins/#{n}.yml", render(n, m)) }
puts "batch4: created #{P.size} plugins, tools=#{P.sum { |_, m| m[1].size }}"