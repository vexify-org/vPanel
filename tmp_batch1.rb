# 批1: 网络诊断 —— 生成 19 个插件，每个 3 个工具 => ~57 工具
P = {
  "net_tcp4" => ["IPv4 TCP 连接诊断", {
    "conns4" => ["列出 IPv4 TCP 活动连接", "ss -tan4 2>/dev/null | head -40"],
    "listen4" => ["列出 IPv4 TCP 监听端口", "ss -tanl4 2>/dev/null | awk 'NR>1{print $4, $6}' | sort -u | head -40"],
    "states4" => ["按状态统计 IPv4 TCP 连接", "ss -tan4 2>/dev/null | awk 'NR>1{print $1}' | sort | uniq -c | sort -rn"],
  }],
  "net_tcp6" => ["IPv6 TCP 连接诊断", {
    "conns6" => ["列出 IPv6 TCP 活动连接", "ss -tan6 2>/dev/null | head -40"],
    "listen6" => ["列出 IPv6 TCP 监听端口", "ss -tanl6 2>/dev/null | awk 'NR>1{print $4, $6}' | sort -u | head -40"],
    "wa6" => ["IPv6 本机全局地址", "ip -6 addr show scope global 2>/dev/null | grep inet6"],
  }],
  "net_udp" => ["UDP 连接与统计诊断", {
    "conns" => ["列出 UDP 连接", "ss -uan 2>/dev/null | head -40"],
    "listeners" => ["列出 UDP 监听端口", "ss -uanl 2>/dev/null | awk 'NR>1{print $4, $5}' | sort -u | head -40"],
    "rxdrop" => ["UDP 收发与丢包统计", "cat /proc/net/snmp | awk '/^Udp:/{print; getline; print}'"],
  }],
  "socket_buf" => ["Socket 收发缓冲诊断", {
    "queues" => ["各连接收发队列(Send-Q/Recv-Q)", "ss -tan 2>/dev/null | head -40"],
    "topq" => ["接收队列积压最多的连接", "ss -tan 2>/dev/null | awk 'NR>1 && $2 + 0 > 0 {print $2, $5, $6}' | sort -rn | head -15"],
    "tcp_mem" => ["TCP 内存压力档位与限制", "cat /proc/sys/net/ipv4/tcp_mem"],
  }],
  "route_v6" => ["IPv6 路由诊断", {
    "routes" => ["列出 IPv6 路由表", "ip -6 route show 2>/dev/null | head -40"],
    "gateway" => ["IPv6 默认网关", "ip -6 route show default 2>/dev/null"],
    "kernel6" => ["内核 IPv6 路由条目", "cat /proc/net/ipv6_route 2>/dev/null | head -30"],
  }],
  "neighbor_table" => ["ARP/邻居表诊断", {
    "arp" => ["ARP 邻居表", "ip neigh | grep -i ether | head -40"],
    "ndp" => ["IPv6 邻居表", "ip -6 neigh show 2>/dev/null | head -40"],
    "anycast" => ["任一状态的邻居(含INCOMPLETE)", "ip neigh 2>/dev/null | head -40"],
  }],
  "conntrack" => ["连接跟踪诊断", {
    "count" => ["当前已跟踪连接数", "cat /proc/sys/net/netfilter/nf_conntrack_count 2>/dev/null || echo 未启用nf_conntrack"],
    "max" => ["连接跟踪上限", "cat /proc/sys/net/netfilter/nf_conntrack_max 2>/dev/null || echo 未启用"],
    "table" => ["已跟踪连接明细", "cat /proc/net/nf_conntrack 2>/dev/null | head -30 || echo 未启用"],
  }],
  "net_softirq" => ["网络软中断诊断", {
    "softirq" => ["各 CPU 软中断计数", "grep -E 'NET_RX|NET_TX|^CPU' /proc/softirqs 2>/dev/null"],
    "retrans" => ["TCP 重传计数", "cat /proc/net/snmp | awk '/^Tcp:/{getline; print \"active=\"$3, \"retrans=\"$22, \"loss=\"$23}'"],
    "overflows" => ["监听队列溢出统计", "nstat -az 2>/dev/null | grep -iE 'ListenOverflows|TCPBacklogDrop' || echo 需iproute2"],
  }],
  "packet_flow" => ["IP/ICMP 报文统计", {
    "ip" => ["IP 层收发与丢弃", "cat /proc/net/snmp | awk '/^Ip:/{getline; print \"rx=\"$9\" tx=\"$11\" drop=\"$12\" bad=\"$13}'"],
    "icmp" => ["ICMP 收发统计", "cat /proc/net/snmp | awk '/^Icmp:/{getline; print \"in=\"$2\" out=\"$11}'"],
    "frag" => ["IP 分片与重组统计", "cat /proc/net/snmp | awk '/^Ip:/{getline; print \"reasm_ok=\"$25\" reasm_fail=\"$26}'"],
  }],
  "interface_errs" => ["网卡错误/丢包深度诊断", {
    "summary" => ["各网卡收发错误/丢包", "ip -s link show 2>/dev/null | grep -E 'e[0-9]|w[0-9]|RX: bytes|TX: bytes|errors|dropped' | head -90"],
    "rx_err" => ["接收错误排名", "grep -v 'lo:' /proc/net/dev | awk 'NR>2{split($1,a,\":\"); if($4+0>0) print a[1], \"rx_err=\"$4}'"],
    "drop_all" => ["收发包丢弃排名", "grep -v 'lo:' /proc/net/dev | awk 'NR>2{split($1,a,\":\"); print a[1], \"rx_drop=\"$5\" tx_drop=\"$13}'"],
  }],
  "tcp_stats" => ["TCP 协议栈统计", {
    "snmp" => ["TCP 连接与收发统计", "cat /proc/net/snmp | awk '/^Tcp:/{getline; print \"active=\"$3\" passive=\"$4\" in=\"$11\" out=\"$12\" retrans=\"$22}'"],
    "abort" => ["TCP 异常中止统计", "cat /proc/net/netstat | awk '/^TcpExt:/{getline; print \"abort=\"$25\" sync=\"$23}'"],
    "keepalive" => ["连接数与 keepalive 设置", "ss -tan | grep -c ESTAB; cat /proc/sys/net/ipv4/tcp_keepalive_time"],
  }],
  "dns_resolve" => ["DNS 解析诊断", {
    "hosts" => ["本地 /etc/hosts 映射", "cat /etc/hosts 2>/dev/null | grep -v '^#' | grep -v '^$'"],
    "servers" => ["当前 DNS 服务器与搜索域", "cat /etc/resolv.conf 2>/dev/null | grep -E 'nameserver|search'"],
    "count" => ["DNS 缓存/解析计数", "grep -c . /etc/hosts 2>/dev/null | awk '{print \"hosts条目=\"$1}'"],
  }],
  "mtu_diag" => ["MTU 链路诊断", {
    "iface_mtu" => ["各网卡 MTU 与链路状态", "ip -br link 2>/dev/null"],
    "default_mtu" => ["默认路由出接口 MTU", "cat /proc/net/route | awk '$1!=\"Iface\"{print $1}' | head -1 | xargs -r -I{} cat /sys/class/net/**/mtu 2>/dev/null; cat $(ls /sys/class/net/*/mtu | head -1) 2>/dev/null"],
    "routes" => ["路由表与 MTU 对照", "ip -r route show 2>/dev/null | head -30"],
  }],
  "http_probe" => ["HTTP/HTTPS 端点探活", {
    "local" => ["本地 HTTP 探活", "curl -s -o /dev/null -w '%{http_code}' --max-time 8 http://127.0.0.1 || echo 0"],
    "status" => ["探测 URL 状态码: 参数 url", "u = arg(\"url\")\nif u == \"\"\n  u = \"http://127.0.0.1\"\nend\nr = http_status(u)\nret(\"状态码: \" + r)"],
    "tlsdate" => ["HTTPS 证书到期: 参数 host", "h = arg(\"host\")\nif h == \"\"\n  h = \"example.com\"\nend\nr = cmd(\"echo | openssl s_client -servername \" + h + \" -connect \" + h + \":443 2>/dev/null | openssl x509 -noout -dates 2>/dev/null\")\nret(trim(r))"],
  }],
  "latency" => ["延迟与丢包诊断", {
    "ping" => ["ping 给定主机: 参数 host、次数", "h = arg(\"host\")\nif h == \"\"\n  h = \"223.5.5.5\"\nend\ncnt = arg(\"count\")\nif cnt == \"\"\n  cnt = \"4\"\nend\nret(cmd(\"ping -c \" + cnt + \" -W 2 \" + h + \" 2>&1 | tail -3\"))"],
    "dns_latency" => ["常用 DNS 延迟对比", "for i in 223.5.5.5 114.114.114.114 8.8.8.8; do echo -n \"$i: \"; ping -c 1 -W 2 $i 2>/dev/null | grep -oE 'time=.*' | head -1 | awk -F= '{print $2\"s\"}'; done"],
    "jitter" => ["网络抖动估算", "ping -c 5 -i 0.2 223.5.5.5 2>/dev/null | tail -2"],
  }],
  "net_drop" => ["网卡丢弃排名", {
    "rx" => ["接收丢弃排名", "grep -v 'lo:' /proc/net/dev | awk 'NR>2{split($1,a,\":\"); print a[1], \"rx_drop=\"$5}' | sort -k2 -t= -rn | head -10"],
    "tx" => ["发送丢弃排名", "grep -v 'lo:' /proc/net/dev | awk 'NR>2{split($1,a,\":\"); print a[1], \"tx_drop=\"$13}' | sort -k2 -t= -rn | head -10"],
    "total" => ["所有网卡收发字节/包", "cat /proc/net/dev | grep -v 'lo:' | awk 'NR>2{split($1,a,\":\"); printf \"%s rx=%sB tx=%sB pkts=%s\\n\", a[1], $2, $11, $3}'"],
  }],
  "bond_links" => ["链路聚合诊断", {
    "bond" => ["Bond 聚合链路状态", "for b in /proc/net/bonding/*; do if [ -f \"$b\" ]; then echo \"== $(basename $b) ==\"; grep -E 'MII|Active|Link' \"$b\"; fi; done 2>/dev/null || echo 无bond"],
    "vlan" => ["VLAN 链路状态", "ip -br link show type vlan 2>/dev/null || echo 无VLAN"],
    "team" => ["Team 链路状态", "ip -br link show type team 2>/dev/null || echo 无team"],
  }],
  "net_device" => ["网卡驱动与队列", {
    "driver" => ["各网卡驱动", "for i in /sys/class/net/*; do if [ -e \"$i/device/driver\" ]; then echo \"$(basename $i): $(basename $(readlink $i/device/driver 2>/dev/null))\"; fi; done 2>/dev/null"],
    "queues" => ["各网卡队列数", "for i in /sys/class/net/*; do q=$(ls $i/queues 2>/dev/null | wc -l); echo \"$(basename $i): $q队列\"; done 2>/dev/null"],
    "speed" => ["各网卡速率/双工", "for i in /sys/class/net/*; do if [ -e $i/speed ] && [ -e $i/duplex ]; then echo \"$(basename $i): $(cat $i/speed)Mb/s $(cat $i/duplex)\"; fi; done 2>/dev/null"],
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
puts "batch1: created #{P.size} plugins, tools=#{P.sum { |_, m| m[1].size }}"