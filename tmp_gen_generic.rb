# 生成 src/generic.rb 所用的内置 MCP 工具表。
# 每个条目: name, desc, schema, cmd_template（支持 %{key} 占位，key ∈ 常见参数）
# 目标：约 200+ 内置工具（补齐至内置 800）。
require 'json'

# 生成器数据：数组元素 = [name, desc, cmd]
# schema 由参数占位自动推导。
E = []
def e(name, desc, cmd); E << [name, desc, cmd]; end

# ---- 系统 / 内核 ----
e "sys_hostname", "主机名与域名", "hostname; hostname -f 2>/dev/null; cat /etc/hostname 2>/dev/null"
e "sys_release", "发行版版本", "cat /etc/os-release 2>/dev/null | head -6"
e "sys_kernel_full", "内核完整信息", "uname -a"
e "sys_arch", "系统架构", "uname -m"
e "sys_boottime", "开机时间", "uptime -s 2>/dev/null; who -b 2>/dev/null"
e "sys_time", "系统当前时间(多种格式)", "date; date -u; date +%s"
e "sys_timezone", "时区与偏移", "cat /etc/timezone 2>/dev/null; date +%Z%z"
e "sys_nginx_uptime", "nginx 进程运行时间", "ps -eo pid,etime,comm | grep [n]ginx | head -3"
e "sys_numa_active", "NUMA 激活状态", "numactl --hardware 2>/dev/null | head -5 || echo 需numactl"
e "sys_nproc_per_core", "每核线程数", "grep 'siblings' /proc/cpuinfo | head -1"
e "sys_cpufreq_avail", "可用调频策略", "cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_available_governors 2>/dev/null"
e "sys_cpu_vendor", "CPU 厂商型号", "grep -m1 'model name' /proc/cpuinfo"
e "sys_cpu_flags_top", "CPU 关键特性标志", "grep -m1 '^flags' /proc/cpuinfo | tr ' ' '\\n' | grep -E '^(sse|avx|vmx|smap|pti)' | head -20"
e "sys_meminfo_raw", "原始内存信息", "cat /proc/meminfo | head -20"
e "sys_swap_avail", "可用交换空间", "free -h | grep -i swap"
e "sys_kernel_params", "内核启动参数", "cat /proc/cmdline"
e "sys_modules_load", "关键模块是否加载", "for m in ip_tables ip6_tables bridge veth; do lsmod | grep -q \"^$m\" && echo \"$m: 已加载\" || echo \"$m: 未加载\"; done"
e "sys_entropy", "熵池大小", "cat /proc/sys/kernel/random/entropy_avail 2>/dev/null"
e "sys_shmall", "共享内存总限额", "cat /proc/sys/kernel/shmall /proc/sys/kernel/shmmax 2>/dev/null"
e "sys_thread_max", "内核线程上限", "cat /proc/sys/kernel/threads-max 2>/dev/null"
e "sys_pid_max", "最大 PID", "cat /proc/sys/kernel/pid_max 2>/dev/null"
e "sys_core_using", "core dump 配置", "cat /proc/sys/kernel/core_pattern 2>/dev/null"
e "sys_exec_shield", "栈保护/ASLR", "cat /proc/sys/kernel/randomize_va_space 2>/dev/null"
e "sys_perf_event_max", "perf 事件上限", "cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null"
e "sys_hostname_subj", "DNS 域设置", "cat /proc/sys/kernel/domainname 2>/dev/null"
e "sys_user_session", "当前登录会话", "who 2>/dev/null | head -20"
e "sys_utmp", "登录历史(非root)", "last -n 20 2>/dev/null || echo 需root或无日志"
e "sys_recent_shutdown", "关机记录", "last -x shutdown reboot 2>/dev/null | head -10 || echo 无记录"

# ---- 进程 / 线程 / 性能 ----
e "proc_top_cpu_full", "CPU 前10(rss/kb 高亮)", "ps -eo pid,user,%cpu,%mem,comm --sort=-%cpu --no-headers | head -10"
e "proc_top_mem_full", "内存前10", "ps -eo pid,user,rss,comm --sort=-rss --no-headers | head -10"
e "proc_children", "指定进程的子进程: 参数 pid", "ps -eo pid,ppid,comm | awk '$2=='%{pid}'{print}' | head -20"
e "proc_parent", "指定进程的父进程: 参数 pid", "ps -o ppid=,comm= -p '%{pid}' 2>/dev/null || echo 无"
e "proc_env", "指定进程环境变量: 参数 pid", "tr '\\0' '\\n' < /proc/'%{pid}'/environ 2>/dev/null | head -30 || echo 需权限"
e "proc_openfiles", "指定进程打开文件: 参数 pid", "ls -l /proc/'%{pid}'/fd 2>/dev/null | head -30 || echo 需权限"
e "proc_threads", "指定进程线程数: 参数 pid", "ls /proc/'%{pid}'/task 2>/dev/null | wc -l"
e "proc_limits", "指定进程资源限制: 参数 pid", "cat /proc/'%{pid}'/limits 2>/dev/null | head -20 || echo 需权限"
e "proc_io", "各进程I/O累计", "cat /proc/[0-9]*/io 2>/dev/null | grep -E 'rchar|wchar' | awk '{print $2}' | head -1 >/dev/null; for p in /proc/[0-9]*; do echo \"$(basename $p): $(awk '/rchar:/{r=$2}/wchar:/{w=$2}END{print \"r=\"r\" w=\"w}' $p/io 2>/dev/null)\"; done 2>/dev/null | sort -t: -k2 -rn | head -10"
e "proc_est_threads", "系统线程总数", "ps -eLf 2>/dev/null | wc -l"
e "proc_est_procs", "进程总数", "ps -e --no-headers 2>/dev/null | wc -l"
e "proc_running", "运行中进程数", "ps -eo stat= --no-headers 2>/dev/null | grep -c '^R'"
e "proc_sleeping", "睡眠进程数", "ps -eo stat= --no-headers 2>/dev/null | grep -c '^S'"
e "proc_zombie", "僵尸进程详情", "ps -eo pid,ppid,stat,comm | awk '$3~/Z/{print}' | head -10 || echo 无僵尸"
e "proc_uninter", "不可中断进程", "ps -eo pid,stat,comm | awk '$2~/D/{print}' | head -10 || echo 无D状态"
e "proc_high_mem_user", "按用户内存占用", "ps -eo user,rss --no-headers | awk '{mem[$1]+=$2} END{for(u in mem) print u, int(mem[u]/1024)\"MB\"}' | sort -k2 -rn | head -10"
e "proc_high_cpu_ppid", "CPU最高进程详情", "ps -eo pid,ppid,user,%cpu,comm --sort=-%cpu --no-headers | head -3"
e "perf_load", "负载与线程对比", "cat /proc/loadavg; nproc"

# ---- 网络 ----
e "net_interfaces_all", "所有网络接口", "ip -br addr 2>/dev/null || ifconfig -a 2>/dev/null"
e "net_ifaddr", "指定接口地址: 参数 iface", "ip addr show '%{iface}' 2>/dev/null || echo 无此接口"
e "net_routes_detail", "详细路由表", "ip route show 2>/dev/null | head -30"
e "net_arp_all", "全部ARP缓存", "ip neigh 2>/dev/null | head -40"
e "net_sockets_stat", "协议栈收发统计", "cat /proc/net/snmp 2>/dev/null | head -12"
e "net_tcp_mem", "TCP内存限制", "cat /proc/sys/net/ipv4/tcp_mem 2>/dev/null"
e "net_rmem", "TCP接收缓冲(r/w)区间", "cat /proc/sys/net/ipv4/tcp_rmem 2>/dev/null"
e "net_wmem", "TCP发送缓冲区间", "cat /proc/sys/net/ipv4/tcp_wmem 2>/dev/null"
e "net_tcp_syncookies", "SYN Cookie 是否开启", "cat /proc/sys/net/ipv4/tcp_syncookies 2>/dev/null"
e "net_tcp_maxsyn", "最大SYN积压", "cat /proc/sys/net/ipv4/tcp_max_syn_backlog 2>/dev/null"
e "net_ipforward4", "IPv4 转发开关", "cat /proc/sys/net/ipv4/ip_forward 2>/dev/null"
e "net_ipforward6", "IPv6 转发开关", "cat /proc/sys/net/ipv6/conf/all/forwarding 2>/dev/null"
e "net_localports", "本地端口范围", "cat /proc/sys/net/ipv4/ip_local_port_range 2>/dev/null"
e "net_estab_count", "已建立连接数", "ss -tan | grep -c ESTAB"
e "net_closed", "待关闭(GO)时间窞统计", "ss -tan | awk '/FIN-WAIT|CLOSE-WAIT/{print $1}' | sort | uniq -c"
e "net_conns_per_ip", "按对端IP连接数", "ss -tan | grep ESTAB | awk '{print $5}' | awk -F: '{print $1}' | sort | uniq -c | sort -rn | head -15"
e "net_top_ip_local", "按本地端口连接数", "ss -tan | grep ESTAB | awk '{print $4}' | awk -F: '{print $NF}' | sort | uniq -c | sort -rn | head -15"
e "net_nic_errors", "网卡错误汇总", "cat /proc/net/dev | awk 'NR>2{if($4+$12>0) split($1,a,\":\"),print a[1],\"err=\"$4+$12}'"
e "net_session_tcp", "TCP监听明细(含进程)", "ss -tlnp 2>/dev/null | head -30"
e "net_dns_servers", "DNS 服务器", "grep -E '^nameserver' /etc/resolv.conf 2>/dev/null"
e "net_hostname_ip", "主机名对应IP", "getent hosts $(hostname) 2>/dev/null"
e "net_route_gw_iface", "默认网关与出口IP", "ip route get 1.1.1.1 2>/dev/null | head -2"

# ---- 磁盘 / 文件 ----
e "disk_usage_full", "分区用量明细", "df -hT 2>/dev/null | head -25"
e "disk_total", "磁盘总量", "df -h --total 2>/dev/null | tail -1"
e "disk_fs_sizes", "文件系统容量", "df -h 2>/dev/null | awk 'NR>1{print $6, $2}' | sort -t= -k2 | head -20"
e "disk_mounted_ro", "只读挂载检查", "mount | awk '$4~/^ro/{print}'"
e "disk_inodes_full", "inode 用量明细", "df -iT 2>/dev/null | head -25"
e "disk_io_util", "磁盘IO利用率(iostat)", "iostat -x 2>/dev/null | tail -20 || cat /proc/diskstats | head -20"
e "disk_latency_await", "磁盘平均响应(await)", "iostat -x 1 2 2>/dev/null | grep -A1 'avg-cpu' | tail -1; iostat -x 1 2 2>/dev/null | awk 'NR>4{print $1, \"await=\"$10\" util=\"$NF}' | head -20 || echo 需sysstat"
e "disk_scsi_errors", "SCSI 层错误", "cat /proc/scsi/scsi 2>/dev/null | head -20"
e "disk_mountpoints", "挂载点列表", "cat /proc/mounts 2>/dev/null | awk '{print $2, $3}' | head -30"
e "disk_uuid_all", "磁盘分区UUID", "blkid 2>/dev/null | head -20 || echo 需root"
e "file_count_dir", "目录文件数: 参数 path", "find '%{path}' -type f 2>/dev/null | wc -l"
e "file_dirs_disk", "目录含子目录列表: 参数 path", "du -h --max-depth=1 '%{path}' 2>/dev/null | sort -h -r | head -15"
e "file_newest", "目录最近文件: 参数 path", "find '%{path}' -type f -printf '%T@ %p\\n' 2>/dev/null | sort -rn | head -10 | cut -d' ' -f2-"
e "file_ctype", "文件类型: 参数 path", "file '%{path}' 2>/dev/null"
e "file_stat", "文件inode信息: 参数 path", "stat '%{path}' 2>/dev/null | head -15"
e "file_leaf", "列出目录: 参数 path", "ls -la '%{path}' 2>/dev/null"
e "file_read_head", "头部20行: 参数 path", "head -20 '%{path}' 2>/dev/null"
e "file_read_tail", "尾部20行: 参数 path", "tail -20 '%{path}' 2>/dev/null"
e "file_bytes", "大小(字节): 参数 path", "stat -c'%s' '%{path}' 2>/dev/null"
e "file_link_target", "符号链接目标: 参数 path", "readlink -f '%{path}' 2>/dev/null"
e "file_crc", "CRC校验: 参数 path", "cksum '%{path}' 2>/dev/null"
e "tmpdir_usage", "/tmp 占用", "du -sh /tmp 2>/dev/null"

# ---- 应用 / 服务 ----
e "svc_listening_procs", "监听进程列表", "ss -tlnp 2>/dev/null | awk 'NR>1{print $1,$4}' | sort -u"
e "svc_failed", "启动失败单元", "systemctl list-units --failed --no-legend 2>/dev/null | head -15 || echo 非systemd"
e "svc_enabled", "开机自启列表", "systemctl list-unit-files --type=service 2>/dev/null | grep enabled | head -20 || echo 非systemd"
e "svc_masked", "被屏蔽服务", "systemctl list-unit-files --type=service 2>/dev/null | grep masked | head -10 || echo 非systemd"
e "svc_active", "running 服务数", "systemctl list-units --type=service --state=running --no-legend 2>/dev/null | wc -l || echo 非systemd"
e "svc_nginx_up", "Nginx 运行状态", "pgrep -x nginx >/dev/null && echo 运行中 || echo 未运行"
e "svc_mysqld_up", "MySQL 运行状态", "pgrep -x mysqld >/dev/null && echo 运行中 || echo 未运行"
e "svc_php_up", "PHP-FPM 运行状态", "pgrep -f php-fpm >/dev/null && echo 运行中 || echo 未运行"
e "svc_redis_up", "Redis 运行状态", "pgrep -x redis-server >/dev/null && echo 运行中 || echo 未运行"
e "svc_nginx_version", "Nginx 版本", "nginx -v 2>&1 || echo 未安装"
e "svc_php_version", "PHP 版本", "php -v 2>/dev/null | head -1 || echo 未安装"
e "svc_mysql_version", "MySQL 版本", "mysql --version 2>/dev/null || mysqld --version 2>/dev/null || echo 未安装"
e "svc_redis_version", "Redis 版本", "redis-server --version 2>/dev/null || echo 未安装"
e "svc_docker_version", "Docker 版本", "docker --version 2>/dev/null || echo 未安装"
e "svc_go_version", "Go 版本", "go version 2>/dev/null || echo 未安装"
e "svc_node_version", "Node 版本", "node -v 2>/dev/null || echo 未安装"
e "svc_python_version", "Python 版本", "python3 --version 2>/dev/null || echo 未安装"
e "svc_java_version", "Java 版本", "java -version 2>&1 | head -1 || echo 未安装"
e "svc_git_version", "Git 版本", "git --version 2>/dev/null || echo 未安装"
e "svc_openssl_version", "OpenSSL 版本", "openssl version 2>/dev/null || echo 未安装"
e "app_nginx_conns", "Nginx活动连接数", "curl -s http://127.0.0.1/nginx_status 2>/dev/null | head -3 || ss -tan | grep :80 | wc -l"
e "app_mysql_listen", "MySQL监听端口", "ss -tlnp 2>/dev/null | grep -E ':3306' | head -2"
e "app_redis_listen", "Redis 监听端口", "ss -tlnp 2>/dev/null | grep -E ':6379' | head -2"

# ---- 包管理 ----
e "pkg_upgradable", "可升级包数", "apt list --upgradable 2>/dev/null | grep -c upgradable || echo 0 或非Debian"
e "pkg_installed_count", "已安装包总数", "dpkg -l 2>/dev/null | grep -c '^ii' || rpm -qa 2>/dev/null | wc -l"
e "pkg_ubuntu_ver", "系统版本代号", "lsb_release -cs 2>/dev/null"
e "pkg_lockfiles", "包管理锁", "ls -l /var/lib/dpkg/lock-frontend /var/lib/apt/lists/lock 2>/dev/null; lsof /var/lib/dpkg/lock* 2>/dev/null | head -5"
e "pkg_held", "标记 hold 的包", "apt-mark showhold 2>/dev/null || echo 无"

# ---- 安全 ----
e "sec_listen_world", "暴露到所有网卡的监听", "ss -tln | awk '$4 ~ /0\\.0\\.0\\.0|\\*:/{print}'"
e "sec_listen_127", "仅本机监听(按127)", "ss -tln | awk '$4 ~ /127\\.0\\.0\\.1|::1:/{print}'"
e "sec_passwd_uids", "系统UID分布", "cat /etc/passwd | awk -F: '$3<1000{print $1,$3,$7}' | head -15"
e "sec_nologin_users", "禁登录账户数", "grep -cE 'nologin|/bin/false' /etc/passwd"
e "sec_sudo_users", "sudo 组成员", "getent group sudo 2>/dev/null || getent group wheel 2>/dev/null"
e "sec_root_login", "root 是否允许ssh密码登录", "grep -iE '^PermitRootLogin' /etc/ssh/sshd_config 2>/dev/null || echo 默认"
e "sec_ssh_port", "SSH 端口", "grep -iE '^Port ' /etc/ssh/sshd_config 2>/dev/null | awk '{print $2}' || echo 22"
e "sec_fail2ban_jails", "fail2ban 监狱状态", "fail2ban-client status 2>/dev/null || echo 未安装"
e "sec_ufw_status", "UFW 防火墙状态", "ufw status 2>/dev/null || echo 未安装"

# ---- 日期/文本/数学/转换 工具 ----
e "util_date_iso", "ISO 时间", "date -Iseconds"
e "util_date_unix", "Unix 时间戳", "date +%s"
e "util_date_utc", "UTC 时间", "date -u"
e "util_date_nextweek", "一周后日期", "date -d '+7 day' +%F"
e "util_text_base64enc", "base64 编码: 参数 text", "printf '%s' '%{text}' | base64"
e "util_text_base64dec", "base64 解码: 参数 text", "printf '%s' '%{text}' | base64 -d 2>/dev/null"
e "util_text_upper", "转大写: 参数 text", "printf '%s' '%{text}' | tr 'a-z' 'A-Z'"
e "util_text_lower", "转小写: 参数 text", "printf '%s' '%{text}' | tr 'A-Z' 'a-z'"
e "util_text_reverse", "反转字符串: 参数 text", "printf '%s' '%{text}' | rev"
e "util_text_len", "字符长度: 参数 text", "printf '%s' '%{text}' | wc -c"
e "util_text_urlenc", "URL 编码: 参数 text", "printf '%s' '%{text}' | jq -sRr @uri 2>/dev/null || python3 -c 'import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))' -- '%{text}' 2>/dev/null || echo 需jq/python"
e "util_math_add", "两数相加: 参数 a b", "echo $(('%{a}' + '%{b}'))"
e "util_math_sub", "两数相减: 参数 a b", "echo $(('%{a}' - '%{b}'))"
e "util_math_mul", "两数相乘: 参数 a b", "echo $(('%{a}' * '%{b}'))"
e "util_math_div", "两数整除: 参数 a b", "test '%{b}' -ne 0 && echo $(('%{a}' / '%{b}')) || echo 除数0"
e "util_math_mod", "两数取模: 参数 a b", "test '%{b}' -ne 0 && echo $(('%{a}' % '%{b}')) || echo 除数0"
e "util_math_abs", "取绝对值: 参数 a", "echo ${a::-1} 2>/dev/null; v='%{a}'; v=\"${v#-}\"; echo \"$v\""
e "util_math_square", "平方: 参数 a", "echo $(( '%{a}' * '%{a}' ))"
e "util_byte_human", "字节转可读: 参数 bytes", "echo '%{bytes}' | awk 'function h(n){if(n>=1073741824)return sprintf(\"%.2fGiB\",n/1073741824);else if(n>=1048576)return sprintf(\"%.2fMiB\",n/1048576);else if(n>=1024)return sprintf(\"%.1fKiB\",n/1024);return sprintf(\"%dB\",n)}{print h($0)}'"
e "util_seconds_hm", "秒转时分: 参数 n", "echo '%{n}' | awk '{h=int($0/3600);m=int(($0%3600)/60);print h\"小时\"m\"分\"}'"
e "util_percent", "百分比: 参数 a b", "test '%{b}' -ne 0 && awk \"BEGIN{printf %.1f%%\\n, (('%{a}')/('%{b}'))*100}\" || echo 分母0"

# ---- 日志 ----
e "log_syslog_head", "syslog 头部", "head -20 /var/log/syslog 2>/dev/null || echo 无syslog"
e "log_nginx_err", "nginx 错误日志尾部", "tail -20 /var/log/nginx/error.log 2>/dev/null || echo 无nginx日志"
e "log_mysql_err", "mysql 错误日志", "tail -20 /var/log/mysql/error.log 2>/dev/null || echo 无"
e "log_auth_fail", "认证失败日志尾部", "grep -i 'failed' /var/log/auth.log 2>/dev/null | tail -15 || echo 无"
e "log_kernel_err", "内核错误日志", "dmesg 2>/dev/null | grep -iE 'error|fail' | tail -15 || echo 需root/无"
e "log_sysctl_audit", "审计日志大小", "ls -lh /var/log/audit/audit.log 2>/dev/null; du -sh /var/log/audit 2>/dev/null"

# ---- 程序化：/proc 与 /sys 只读工具（批量）----
PROC_READS = {
  "proc_loadavg" => "系统平均负载", "proc_meminfo" => "内存明细",
  "proc_cpuinfo" => "CPU 信息", "proc_partitions" => "磁盘分区",
  "proc_diskstats" => "磁盘 IO 统计", "proc_netdev" => "网卡流量",
  "proc_nettcp" => "TCP 连接表", "proc_netudp" => "UDP 连接表",
  "proc_vmstat" => "内存虚拟统计", "proc_zoneinfo" => "内存区域",
  "proc_softirqs" => "软中断", "proc_interrupts" => "硬中断",
  "proc_iomem" => "I/O 内存映射", "proc_ioports" => "I/O 端口",
  "proc_modules" => "已加载模块", "proc_filesystems" => "文件系统类型",
  "proc_mounts" => "当前挂载", "proc_swaps" => "交换设备",
  "proc_stat" => "整体CPU统计", "proc_buddyinfo" => "伙伴内存空闲页",
  "proc_pagetypeinfo" => "页类型分布", "proc_cgroups" => "cgroup 控制器",
  "proc_schedstat" => "调度统计", "proc_timer_list" => "定时器",
  "proc_uptime" => "运行时长", "proc_slabinfo" => "内核缓存(slab)",
}
PROC_READS.each do |name, desc|
  fn = name.sub(/\Aproc_/, "/proc/")
  e name, "#{desc}", "head -40 /proc/#{name.sub('proc_','')} 2>/dev/null || echo 需权限"
end
SYS_READS = {
  "sys_cpufreq_curr" => [ "当前CPU频率", "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq" ],
  "sys_cpufreq_max" => [ "CPU最大频率", "/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq" ],
  "sys_cpufreq_min" => [ "CPU最小频率", "/sys/devices/system/cpu/cpu0/cpufreq/scaling_min_freq" ],
  "sys_cpu_nice" => [ "CPU nice 模式", "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor" ],
  "sys_thermal_all" => [ "全部热区", "/sys/class/thermal" ],
  "sys_vm_minfree" => [ "最小空闲页", "/proc/sys/vm/min_free_kbytes" ],
}
e "df_root", "根分区用量", "df -h / | tail -1"
e "free_all", "全部内存", "free -h"
e "iostat_all", "IO 统计全部", "iostat 2>/dev/null || echo 需sysstat"
e "mpstat_all", "每核CPU", "mpstat -P ALL 1 2 2>/dev/null | tail -15 || echo 需sysstat"
e "pidstat_all", "每进程CPU", "pidstat 1 1 2>/dev/null | tail -15 || echo 需sysstat"
e "sar_all", "历史CPU", "sar -u 2>/dev/null | tail -5 || echo 需sysstat"
e "vmstat_all", "虚拟内存统计", "vmstat 1 2 2>/dev/null | tail -2"
e "lsof_ports", "端口占用进程", "lsof -i -n -P 2>/dev/null | head -20 || echo 需lsof"
e "netstat_conns", "全部连接", "netstat -ant 2>/dev/null | head -30 || ss -ant"
e "route_table", "全部路由", "route -n 2>/dev/null | head -20"
e "dmesg_all", "内核日志", "dmesg 2>/dev/null | tail -30 || echo 需root"
e "lsblk_all", "块设备树", "lsblk 2>/dev/null || cat /proc/partitions"
e "lspci_short", "PCI 设备", "lspci 2>/dev/null || echo 需pciutils"
e "lsusb_short", "USB 设备", "lsusb 2>/dev/null || echo 需usbutils"
e "dmidecode_short", "硬件型号", "dmidecode -t system 2>/dev/null | grep -E 'Manufacturer|Product|Serial' || echo 需root"
e "ps_aux_long", "全部进程详表", "ps aux --sort=-%cpu 2>/dev/null | head -25"
e "top_short", "实时 top", "top -bn1 2>/dev/null | head -20"
e "free_detail", "内存详细", "free -ht 2>/dev/null"
e "uptime_detail", "开机+负载", "uptime"
e "date_epoch_now", "当前时间戳", "date +%s"
e "hostname_detail", "主机名详情", "hostnamectl 2>/dev/null | head -10 || hostname"
e "env_current", "环境变量", "env | sort | head -25"

# + 补齐数量：更多通用诊断工具
e "diag_port_open_check", "本机端口连通: 参数 host port", "timeout 2 bash -c 'echo > /dev/tcp/'%{host}'/'%{port}'' && echo 开放 || echo 关闭"
e "diag_http_code", "URL 返回码: 参数 url", "curl -s -o /dev/null -w '%{http_code}' --max-time 8 '%{url}' 2>/dev/null || echo 0"
e "diag_http_cert_expire", "URL 证书到期: 参数 url", "echo | timeout 5 openssl s_client -servername '%{url}' -connect '%{url}':443 2>/dev/null | openssl x509 -noout -enddate 2>/dev/null || echo 无法获取"
e "diag_dns_parse", "解析域名: 参数 host", "getent ahosts '%{host}' 2>/dev/null | head -10 || echo 解析失败"
e "diag_ping_n", "ping 测试: 参数 host", "ping -c 4 -W 2 '%{host}' 2>&1 | tail -3"
e "diag_traceroute", "路由跟踪: 参数 host", "traceroute -m 12 '%{host}' 2>/dev/null | head -14 || echo 需traceroute"
e "diag_ssl_tlsv", "TLS 版本探测: 参数 host", "openssl s_client -connect '%{host}':443 -tls1_2 2>&1 | grep -E 'Protocol|Cipher' | head -3"
e "diag_whois_ip", "本地出口公网IP", "curl -s --max-time 5 ifconfig.me 2>/dev/null || curl -s --max-time 5 ip.sb 2>/dev/null || echo 无法获取"
e "diag_cpu_cores", "逻辑核数", "nproc"
e "diag_memory_gb", "内存总量(GB)", "free -g | awk 'NR==2{print $2\"GB\"}'"
e "diag_uptime", "开机时长", "uptime -p 2>/dev/null || uptime"
e "diag_disk_free_gb", "根分区可用(GB)", "df -BG / | tail -1 | awk '{print $4} 可用G'"
e "diag_swap_used", "交换使用量", "free -m | grep -i swap | awk '{print $3\"MB已用\"}'"

# 构建输出
def schema_for(cmd)
  keys = cmd.scan(/%\{([a-z_]+)\}/).flatten.uniq
  return "{}" if keys.empty?
  props = keys.map { |k| "\"#{k}\":{\"type\":\"string\"}" }.join(",")
  "{\"#{keys.map { |k| "#{k}":{\"type":"string"}" }.join(',')}\"}"  # placeholder
end

# 直接构造 schema
def schema_for2(cmd)
  keys = cmd.scan(/%\{([a-z_]+)\}/).flatten.uniq
  return "{}" if keys.empty?
  "{" + keys.map { |k| "\"#{k}\":{\"type\":\"string\"}" }.join(",") + "}"
end

# Rust 源码输出
out = "//! 内置 MCP 工具表（通用命令风格）。
//! 每个工具对应一条真实 shell 命令，用于 MCP `tools/list` 与 `tools/call` 兜底分发。
//! 自动生成，勿手改。\n\n"
out << "pub struct G { pub name: &'static str, pub desc: &'static str, pub schema: &'static str, pub cmd: &'static str }\n\n"
out << "pub const GENERIC: &[G] = &[\n"
E.each do |name, desc, cmd|
  out << "    G{name: \"#{name}\", desc: \"#{desc.gsub('"', '\\"')}\", schema: \"#{schema_for2(cmd)}\", cmd: r##\"#{cmd.gsub('"', '\\"').gsub('#{', '\u0023{')}\"##},\n"
end
out << "];\n"

# run() 函数：查表、填充参数、执行 shell。
out << <<~RUST
pub fn run(name: &str, args: &str) -> Option<(bool, String)> {
    let ent = GENERIC.iter().find(|g| g.name == name)?;
    let filled = fill(ent.cmd, args);
    match std::process::Command::new("sh").arg("-c").arg(filled).output() {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).trim_end().trim().to_string();
            if !o.stderr.is_empty() && s.is_empty() {
                s = String::from_utf8_lossy(&o.stderr).trim_end().trim().to_string();
            }
            Some((true, s))
        }
        Err(e) => Some((false, format!("执行失败: {}", e))),
    }
}

fn fill(tmpl: &str, args: &str) -> String {
    let mut s = tmpl.to_string();
    for k in ["url","host","port","path","file","text","name","n","a","b","bytes","iface","pid"] {
        let key = format!("%{{{}}}", k);
        if s.contains(&key) {
            let v = arg_str(args, k);
            s = s.replace(&key, &v);
        }
    }
    s
}

fn arg_str(args: &str, key: &str) -> String {
    let needle = format!("\"{}\":", key);
    let idx = match args.find(&needle) { Some(i) => i + needle.len(), None => return String::new() };
    let rest = args[idx..].trim_start();
    if rest.starts_with('"') {
        let i = match rest[1..].find('"') { Some(j) => j, None => return String::new() };
        rest[1..1 + i].to_string()
    } else {
        let e = rest.find(|c: char| c == ',' || c == '}').unwrap_or(rest.len());
        rest[..e].trim().to_string()
    }
}
RUST

File.write("/workspace/src/generic.rb.gen", out)
puts "generated entries=#{E.size} target=/workspace/src/generic.rb.gen"