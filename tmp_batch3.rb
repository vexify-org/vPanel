# 批3: 存储/文件/内核调度 —— 19 个插件，每个 3 工具 => 57 工具
P = {
  "zfs_pool" => ["ZFS 存储池诊断", {
    "pools" => ["ZFS 池状态", "zpool status 2>/dev/null || echo 未安装zfs"],
    "datasets" => ["ZFS 数据集与配额", "zfs list 2>/dev/null | head -30 || echo 未安装zfs"],
    "scrub" => ["ZFS 校验进度", "zpool status 2>/dev/null | grep -iE 'scan|scrub' || echo 未安装zfs"],
  }],
  "btrfs_files" => ["Btrfs 文件系统诊断", {
    "usage" => ["Btrfs 文件系统用量", "btrfs filesystem usage / 2>/dev/null || btrfs filesystem df / 2>/dev/null || echo 非btrfs"],
    "devices" => ["Btrfs 设备列表", "btrfs filesystem show 2>/dev/null || echo 非btrfs"],
    "scrub" => ["Btrfs 校验状态", "btrfs scrub status / 2>/dev/null || echo 非btrfs"],
  }],
  "mdraid_arrs" => ["软RAID(MD)阵列诊断", {
    "status" => ["RAID 阵列状态", "cat /proc/mdstat 2>/dev/null | head -30 || echo 无软RAID"],
    "parity" => ["RAID 重建/一致性", "cat /proc/mdstat 2>/dev/null | grep -iE 'resync|recovery|check' || echo 无进行中任务"],
    "devices" => ["RAID 成员设备", "for md in /dev/md*; do [ -b \"$md\" ] && echo \"== $md ==\"; mdadm --detail \"$md\" 2>/dev/null | grep -E 'State|active|Working|Rebuild' | head -5; done 2>/dev/null || echo 无"],
  }],
  "lvm_volumes" => ["LVM 逻辑卷诊断", {
    "pvs" => ["物理卷", "pvs 2>/dev/null || echo 未安装lvm2"],
    "lvs" => ["逻辑卷", "lvs 2>/dev/null || echo 未安装lvm2"],
    "vg" => ["卷组可用空间", "vgs 2>/dev/null || echo 未安装lvm2"],
  }],
  "nfs_mounts" => ["NFS 挂载诊断", {
    "exports" => ["NFS 导出目录", "cat /etc/exports 2>/dev/null | grep -v '^#' | grep -v '^$' | head -20 || echo 无exports"],
    "mounts" => ["NFS 挂载点", "mount 2>/dev/null | grep -i nfs || echo 无NFS挂载"],
    "stats" => ["NFS 客户端统计", "cat /proc/self/mountstats 2>/dev/null | head -40 || echo 需要内核支持"],
  }],
  "dmesg_tail" => ["内核日志实时诊断", {
    "tail" => ["内核日志最近20行", "dmesg 2>/dev/null | tail -20 || echo 需root"],
    "errors" => ["内核日志中的错误", "dmesg 2>/dev/null | grep -iE 'error|fail|bug|oops|panic' | tail -20 || echo 需root或无错误"],
    "usb" => ["内核日志 USB 事件", "dmesg 2>/dev/null | grep -i usb | tail -15 || echo 需root或无USB事件"],
  }],
  "page_stats" => ["内存页分配统计诊断", {
    "pages" => ["页分配/释放", "grep -E 'pgalloc|pgfree|pgfault|pgmajfault' /proc/vmstat"],
    "thp" => ["透明大页统计", "grep -E 'thp_|_thp' /proc/vmstat 2>/dev/null | head -20 || echo 内核无thp统计"],
    "buddy" => ["伙伴系统各阶空闲页", "cat /proc/buddyinfo 2>/dev/null | head -10 || echo 需读权限"],
  }],
  "slab_usage" => ["内核缓存(Slab)利用率诊断", {
    "total" => ["Slab 总量", "grep -E '^Slab|^SReclaimable|^SUnreclaim' /proc/meminfo"],
    "shrink" => ["可回收 slab", "cat /sys/kernel/slab/shrink_cache 2>/dev/null; echo 需root查看可回收量"],
    "objs" => ["对象数最多 slab", "cat /proc/slabinfo | awk 'NR>2{print $4, $1}' | sort -rn | head -10"],
  }],
  "file_locks" => ["文件锁诊断", {
    "flocks" => ["活动文件锁", "cat /proc/locks 2>/dev/null | head -30"],
    "by_proc" => ["占用锁最多的进程", "cat /proc/locks 2>/dev/null | awk 'NR>1{print $NF}' | sort | uniq -c | sort -rn | head -10"],
    "nfs_locks" => ["NFS 相关锁", "cat /proc/locks 2>/dev/null | grep -i nfs | head -10"],
  }],
  "ipc_queue" => ["IPC 通信诊断", {
    "shm" => ["共享内存段", "cat /proc/sysvipc/shm 2>/dev/null | head -20"],
    "sems" => ["信号量集", "cat /proc/sysvipc/sem 2>/dev/null | head -20"],
    "msgs" => ["消息队列", "cat /proc/sysvipc/msg 2>/dev/null | head -20"],
  }],
  "file_descs" => ["文件描述符诊断", {
    "proc_fd" => ["fd 占用最多的进程: 参数 top", "t = arg(\"top\")\nif t == \"\"\n  t = \"10\"\nend\nret(cmd(\"for p in /proc/[0-9]*; do n=$(ls $p/fd 2>/dev/null | wc -l); if [ \\\"$n\\\" -gt 0 ]; then echo \\\"$(basename $p) fds=$n\\\"; fi; done 2>/dev/null | sort -t= -k2 -rn | head -\" + t))"],
    "limit" => ["系统 fd 上限", "cat /proc/sys/fs/file-max; cat /proc/sys/fs/nr_open"],
    "cur" => ["当前已用 fd", "cat /proc/sys/fs/file-nr"],
  }],
  "io_throttle" => ["IO 节流/配额诊断", {
    "blkio" => ["cgroup 块IO节流", "cat /sys/fs/cgroup/io.max 2>/dev/null || echo 非cgroup v2 / 无限制"],
    "io_stat" => ["各磁盘读写", "iostat -d 2>/dev/null | head -20 || cat /proc/diskstats | head -20"],
    "iops" => ["各盘 IOPS", "grep -E 'sd[a-z] ' /proc/diskstats | awk '{print $3, \"readIO=\"$4\" writeIO=\"$8}'"],
  }],
  "process_sched" => ["进程调度延迟诊断", {
    "loadavg" => ["运行队列与负载", "cat /proc/loadavg"],
    "rt_procs" => ["实时调度进程", "ps -eo pid,comm,rtprio --no-headers 2>/dev/null | awk '$3!=\"-\"{print}' | head -20 || echo 无实时进程"],
    "nice_stats" => ["各优先级进程数", "ps -eo nice --no-headers 2>/dev/null | sort -n | uniq -c | head -15"],
  }],
  "cgroup_slice" => ["cgroup 资源组诊断", {
    "slices" => ["systemd 资源组", "systemd-cgtop --iterations=1 2>/dev/null | head -20 || echo 需systemd-cgtop"],
    "mem_current" => ["当前 cgroup 内存", "cat /sys/fs/cgroup/memory.current 2>/dev/null | awk '{print \"当前内存=\"$1/1024/1024\"MB\"}'"],
    "cpu_usage" => ["当前 cgroup CPU", "cat /sys/fs/cgroup/cpu.stat 2>/dev/null | head -5"],
  }],
  "kernel_workers" => ["内核工作队列诊断", {
    "workqueue" => ["工作队列概览", "cat /proc/workqueue 2>/dev/null | head -20 || echo 内核未开放"],
    "kthreads" => ["内核线程数", "ps -eo comm --no-headers 2>/dev/null | grep -c '^k' | awk '{print \"k*内核线程=\"$1}'"],
    "softlock" => ["软锁检测状态", "cat /proc/sys/kernel/watchdog 2>/dev/null | awk '{print \"watchdog=\"$1}'"],
  }],
  "mount_audit" => ["挂载选项安全审计", {
    "suidopts" => ["含 suid 选项挂载", "mount 2>/dev/null | grep -iE '\\(rw|suid' | grep -v 'no.*suid' | head -20"],
    "devsuid" => ["dev/suid 挂载点", "mount 2>/dev/null | grep -iE 'exec|suid' | grep -viE 'nosuid|noexec|proc|sysfs|cgroup' | head -20"],
    "oversize" => ["特殊文件系统挂载", "mount 2>/dev/null | grep -viE 'cgroup|proc|sysfs|tmpfs|devpts|securityfs|pstore|mqueue|configfs|debugfs|hugetlbfs' | head -30"],
  }],
  "rotate_logs" => ["日志轮转诊断", {
    "logrotate_cfg" => ["logrotate 配置概览", "grep -rh '^[a-zA-Z0-9_/]' /etc/logrotate.d/ 2>/dev/null | grep -v '^{' | head -30 || echo 无配置"],
    "state" => ["logrotate 状态", "cat /var/lib/logrotate.status 2>/dev/null | head -20 || echo 无状态文件"],
    "oversized" => ["超大日志文件", "find /var/log -type f -size +200M 2>/dev/null | head -15"],
  }],
  "disk_health" => ["磁盘健康监测", {
    "smart" => ["SMART 健康状态", "smartctl -H -A /dev/sda 2>/dev/null | grep -E 'Result|Reallocat|Current_Pending|Offline' | head -12 || echo 需smartctl"],
    "realloc" => ["重映射扇区", "for d in $(ls /sys/block/sd? 2>/dev/null); do echo \"$(basename $d): $(grep -c . $d/device/state 2>/dev/null)\"; done 2>/dev/null"],
    "dmesg_err" => ["磁盘I/O错误", "dmesg 2>/dev/null | grep -iE 'ext4-fs error|I/O error|EIO' | tail -10 || echo 需root或无错误"],
  }],
  "tmpfs_usage" => ["内存文件系统(tmpfs)诊断", {
    "tmpfs" => ["tmpfs 挂载与用量", "df -hT 2>/dev/null | grep -i tmpfs | head -15"],
    "devshm" => ["/dev/shm 用量", "df -h /dev/shm 2>/dev/null | tail -1"],
    "shm_limit" => ["各 tmpfs 限高", "mount 2>/dev/null | grep tmpfs | head -15"],
  }],
  "fs_io_stats" => ["文件系统 I/O 统计诊断", {
    "request" => ["各盘 I/O 请求队列", "cat /proc/diskstats 2>/dev/null | grep -E 'sd[a-z] |nvme[0-9]n' | head -20"],
    "sectors" => ["读写扇区数", "cat /proc/diskstats 2>/dev/null | grep -E 'sd[a-z] |nvme[0-9]n' | awk '{print $3, \"read_sect=\"$6\" write_sect=\"$10}'"],
    "merged" => ["合并IO统计", "cat /proc/diskstats 2>/dev/null | grep -E 'sd[a-z] |nvme[0-9]n' | awk '{print $3, \"read_merge=\"$5\" write_merge=\"$9}'"],
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
puts "batch3: created #{P.size} plugins, tools=#{P.sum { |_, m| m[1].size }}"