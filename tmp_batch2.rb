# 批2: 硬件/电源/内核诊断 —— 18 个插件，每个 3 工具 => 54 工具
P = {
  "cpu_cstates" => ["CPU 睡眠态(C-states)诊断", {
    "states" => ["各 C 状态停留次数", "grep -E '^C[0-9]' /sys/devices/system/cpu/cpu0/cpuidle/state*/name 2>/dev/null | head -30"],
    "usage" => ["各 C 状态使用时间", "cat /sys/devices/system/cpu/cpu0/cpuidle/state*/usage 2>/dev/null | paste -sd' ' | awk '{print \"usage=\"$0}'"],
    "latency" => ["各 C 状态唤醒延迟", "for i in /sys/devices/system/cpu/cpu0/cpuidle/state*; do [ -e $i/latency ] && echo \"$(basename $i) latency=$(cat $i/latency)\"; done 2>/dev/null"],
  }],
  "cpu_freq_hw" => ["CPU 频率在线诊断", {
    "scaling" => ["各核当前/最大频率", "grep MHz /proc/cpuinfo | sed 's/^cpu MHz/cpu MHz/' | head -$(nproc)"],
    "governor_all" => ["各核调频策略", "for c in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do echo \"$(basename $(dirname $c)): $(cat $c)\"; done 2>/dev/null | sort -u"],
    "thermal_throttle" => ["频率受限计数器", "grep -A2 'ttm throttle' /proc/cpuinfo 2>/dev/null; grep -A2 'Throttle' /proc/cpuinfo 2>/dev/null"],
  }],
  "cpu_topology" => ["CPU 拓扑诊断", {
    "threads" => ["每核 vCPU 拓扑", "grep -E '^processor|^physical id|^core id' /proc/cpuinfo | head -60"],
    "sockets" => ["物理 CPU 数与核数", "nproc; grep -c '^physical id' /proc/cpuinfo; grep -c '^core id' /proc/cpuinfo"],
    "cache" => ["各核缓存大小", "lscpu 2>/dev/null | grep -iE 'Cache|Socket|Core|Thread' | head -20"],
  }],
  "memory_slab" => ["内核内存(Slab)诊断", {
    "top_slab" => ["Slab 占用前10", "cat /proc/slabinfo | awk 'NR>2{print $3, $1}' | sort -rn | head -10"],
    "slab_total" => ["Slab 总量与利用率", "grep -E '^slab' /proc/meminfo"],
    "vmstat_paging" => ["内存分页交换活动", "grep -E 'pgpgin|pgpgout|pgmajfault|pswpin|pswpout' /proc/vmstat"],
  }],
  "sysctl_vm" => ["内存/Swap 内核参数诊断", {
    "overcommit" => ["Overcommit 策略与比例", "cat /proc/sys/vm/overcommit_memory /proc/sys/vm/overcommit_ratio 2>/dev/null"],
    "dirty" => ["脏页回写阈值", "cat /proc/sys/vm/dirty_ratio /proc/sys/vm/dirty_background_ratio 2>/dev/null"],
    "swapness" => ["交换倾向与可用性相关", "cat /proc/sys/vm/swappiness 2>/dev/null | awk '{print \"swappiness=\"$1}'"],
  }],
  "sysctl_net" => ["网络内核参数诊断", {
    "tcp_tuning" => ["TCP 收发缓冲区间", "cat /proc/sys/net/core/rmem_max /proc/sys/net/core/wmem_max 2>/dev/null"],
    "inotify" => ["文件监视上限", "cat /proc/sys/fs/inotify/max_user_watches 2>/dev/null | awk '{print \"max_user_watches=\"$1}'"],
    "sockets_max" => ["进程资源-打开文件上限", "ulimit -n"],
  }],
  "kernel_modules" => ["内核模块诊断", {
    "list" => ["已加载模块", "lsmod | head -40"],
    "params" => ["模块加载参数: 参数 module", "m = arg(\"module\")\nif m == \"\"\n  m = \"ip_tables\"\nend\nr = cmd(\"find /sys/module/\" + m + \"/parameters -type f 2>/dev/null | head -5 | sed 's/.*parameters\\///' | head -5; ls /sys/module/\" + m + \"/parameters 2>/dev/null\")\nret(\"模块参数: \" + trim(r))"],
    "tainted" => ["内核是否被污染", "cat /proc/sys/kernel/tainted 2>/dev/null | awk '{print \"tainted=\"$1}'"],
  }],
  "kernel_times" => ["内核时间与定时器诊断", {
    "hrtimer" => ["高精度定时器信息", "cat /proc/timer_list 2>/dev/null | head -5 || echo 需要内核配置"],
    "uptime" => ["系统运行时长", "cat /proc/uptime"],
    "clock" => ["时钟源与跳变次数", "dmesg 2>/dev/null | grep -i 'clocksource' | tail -3 || cat /sys/devices/system/clocksource/clocksource*/current_clocksource 2>/dev/null"],
  }],
  "irq_count" => ["中断计数诊断", {
    "top_irq" => ["触发最多中断", "awk 'NR>1{sum=0; for(i=2;i<=NF;i++) sum+=$i; print sum, $1}' /proc/interrupts | sort -rn | head -12"],
    "by_cpu" => ["中断在各 CPU 分布", "cat /proc/interrupts | head -15"],
    "name" => ["中断名映射", "grep -E '^[0-9]+:' /proc/interrupts | awk '{print $1, $NF}' | head -20"],
  }],
  "thermal_zone" => ["内核热区温度诊断", {
    "temperatures" => ["各热区当前温度", "for z in /sys/class/thermal/thermal_zone*; do [ -e $z/temp ] && echo \"$(basename $z) type=$(cat $z/type 2>/dev/null) temp=$(awk '{print $1/1000\"C\"}' $z/temp)\"; done 2>/dev/null"],
    "trip" => ["热区触发温度", "for z in /sys/class/thermal/thermal_zone*; do [ -e $z/trip_point_0_temp ] && echo \"$(basename $z) trip0=$(awk '{print $1/1000\"C\"}' $z/trip_point_0_temp)\"; done 2>/dev/null"],
    "cooling" => ["冷却设备状态", "for c in /sys/class/thermal/cooling_device*; do echo \"$(basename $c): $(cat $c/type 2>/dev/null) $(cat $c/cur_state 2>/dev/null)/$(cat $c/max_state 2>/dev/null)\"; done 2>/dev/null | head -20"],
  }],
  "power_supply" => ["电源/电池状态诊断", {
    "supplies" => ["电源设备概览", "for p in /sys/class/power_supply/*; do echo \"$(basename $p): $(cat $p/type 2>/dev/null) status=$(cat $p/status 2>/dev/null)\"; done 2>/dev/null"],
    "ac" => ["AC 电源状态", "cat /sys/class/power_supply/AC/online 2>/dev/null | awk '{print \"AC在线=\"$1\"(1在线)\"}'"],
    "battery" => ["电池电量", "for p in /sys/class/power_supply/BAT*; do [ -e $p/capacity ] && echo \"$(basename $p): $(cat $p/capacity)%%\"; done 2>/dev/null || echo 无电池"],
  }],
  "dmi_info" => ["硬件型号信息诊断", {
    "board" => ["主板/机型", "dmidecode -t baseboard -t system 2>/dev/null | grep -E 'Manufacturer|Product|Version|Serial' | head -12 || echo 需要root"],
    "bios" => ["BIOS 版本", "dmidecode -t bios 2>/dev/null | grep -E 'Vendor|Version|Date' | head -6 || echo 需要root"],
    "memory" => ["内存条信息", "dmidecode -t memory 2>/dev/null | grep -E 'Size:|Type:|Speed:|Rank:' | head -16 || echo 需要root"],
  }],
  "cpu_vulnerabilities" => ["CPU 安全漏洞缓解诊断", {
    "meltdown" => ["Meltdown/Spectre 缓解状态", "grep . /sys/devices/system/cpu/vulnerabilities/* 2>/dev/null | head -20"],
    "pcid" => ["PCID/IKI 特性", "grep -oE '\\(.*KPTI.*\\)|pti|pcid' /proc/cpuinfo | head -1"],
    "microcode" => ["CPU 微码版本", "grep 'microcode' /proc/cpuinfo | head -1"],
  }],
  "iommu_groups" => ["IOMMU/直通分组诊断", {
    "groups" => ["IOMMU 分组与设备", "for g in /sys/kernel/iommu_groups/*; do echo -n \"$(basename $g): \"; for d in $g/devices/*; do echo -n \"$(basename $d) \"; done; echo; done 2>/dev/null | head -30 || echo 未启用IOMMU"],
    "enabled" => ["IOMMU 是否启用", "dmesg 2>/dev/null | grep -iE 'DMAR|IOMMU' | head -5 || echo 需root查看dmesg"],
    "vfio" => ["直通驱动使用", "lspci -nnk 2>/dev/null | grep -B1 -A1 'vfio-pci' | head -30 || echo 需pciutils"],
  }],
  "scsi_disk" => ["磁盘/SCSI设备诊断", {
    "scsi_dev" => ["SCSI 设备列表", "lsscsi 2>/dev/null || cat /proc/scsi/scsi 2>/dev/null || echo 需lsscsi"],
    "disk_model" => ["磁盘型号/序列号", "for d in /sys/block/sd?; do echo \"$(basename $d): $(cat $d/device/model 2>/dev/null) $(cat $d/device/vendor 2>/dev/null)\"; done 2>/dev/null"],
    "rotational" => ["机械/固态判断", "for d in /sys/block/sd?; do echo \"$(basename $d): rotational=$(cat $d/queue/rotational 2>/dev/null)\"; done 2>/dev/null"],
  }],
  "block_map" => ["块设备拓扑诊断", {
    "disks" => ["块设备概览", "lsblk 2>/dev/null | head -40 || cat /proc/partitions"],
    "ra" => ["各盘读提前量", "for d in /sys/block/sd?; do echo \"$(basename $d): ra=$(cat $d/queue/read_ahead_kb 2>/dev/null)K\"; done 2>/dev/null"],
    "io_sched" => ["各盘 I/O 调度器", "for d in /sys/block/sd?; do echo \"$(basename $d): $(cat $d/queue/scheduler 2>/dev/null)\"; done 2>/dev/null"],
  }],
  "pci_slots" => ["PCI 扩展槽/链路诊断", {
    "link_width" => ["PCIe 链路宽速", "lspci -vv 2>/dev/null | grep -E 'LnkSta|^[0-9]' | head -40 || echo 需pciutils"],
    "errors" => ["PCI 错误计数", "echo '需查看lspci -vv AER'; for e in /sys/bus/pci/devices/*/aer_dev_cor_err; do [ -e $e ] && echo \"$e: $(cat $e)\"; done 2>/dev/null | head -10"],
    "bus_top" => ["PCI 设备总线拓扑", "lspci -t 2>/dev/null | head -30 || echo 需pciutils"],
  }],
  "virt_devices" => ["虚拟化/容器设备诊断", {
    "virt_type" => ["运行环境类型", "systemd-detect-virt 2>/dev/null || echo none"],
    "cgroup" => ["cgroup 版本", "ls /sys/fs/cgroup/cgroup.controllers 2>/dev/null | wc -c | awk '{print \"cgroup v2(single-file)\"}'; stat -fc %T /sys/fs/cgroup 2>/dev/null"],
    "container" => ["是否容器与容器引擎", "grep -sq control /proc/1/cgroup 2>/dev/null && echo 容器内 || echo 非容器; cat /proc/1/cgroup 2>/dev/null | head -3"],
  }],
  "numa_topology" => ["NUMA 拓扑诊断", {
    "nodes" => ["NUMA 节点内存分布", "numactl --hardware 2>/dev/null || cat /sys/devices/system/node/node*/meminfo 2>/dev/null | grep 'MemTotal' | head -10"],
    "membind" => ["当前进程 NUMA 绑定", "numactl --show 2>/dev/null || echo 需numactl"],
    "zoneinfo" => ["各节点内存区域", "grep -E '^Node|normal|DMA' /proc/zoneinfo 2>/dev/null | head -40"],
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
puts "batch2: created #{P.size} plugins, tools=#{P.sum { |_, m| m[1].size }}"