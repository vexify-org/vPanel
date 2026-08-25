# 批5: 补充插件 —— 4 个插件，每个 3-4 工具
P = {
  "proc_watchdog" => ["进程存活与守护诊断", {
    "top_uptime" => ["存活最久的进程", "ps -eo pid,etimes,comm --sort=-etimes --no-headers 2>/dev/null | head -8"],
    "restarts" => ["重启最频繁进程", "ps -eo pid,etime,comm --no-headers 2>/dev/null | sort -k2 | head -8"],
    "defunct" => ["僵尸进程", "ps -eo pid,ppid,stat,comm --no-headers 2>/dev/null | awk '$3 ~ /Z/{print}' | head -10 || echo 无僵尸进程"],
  }],
  "cron_next" => ["计划任务下一次执行诊断", {
    "at" => ["at 一次性任务", "atq 2>/dev/null | head -10 || echo 无at任务"],
    "systemd_timers" => ["systemd 定时器下次时间", "systemctl list-timers --no-legend 2>/dev/null | head -12 || echo 非systemd"],
    "cron_syntax" => ["cron 语法问题检查", "cat /etc/crontab 2>/dev/null | grep -vE '^#|^$' | head -15"],
  }],
  "kernel_message" => ["内核消息风暴诊断", {
    "rate" => ["近期内核消息速率", "dmesg 2>/dev/null | wc -l | awk '{print \"内核log条数=\"$1}' || echo 需root"],
    "repeats" => ["重复消息统计", "dmesg 2>/dev/null | tail -200 | sort | uniq -c | sort -rn | head -12 || echo 需root"],
    "warn" => ["内核警告", "dmesg 2>/dev/null | grep -iE 'WARNING|BUG:' | tail -10 || echo 需root或无告警"],
  }],
  "shop_status" => ["面板商店与更新诊断", {
    "store" => ["商店清单可达性", "curl -s -o /dev/null -w '%{http_code}' --max-time 8 https://raw.githubusercontent.com 2>/dev/null || echo 0"],
    "store_health" => ["软件源连通性", "grep -rhE '^deb ' /etc/apt/sources.list* 2>/dev/null | head -3 | awk '{print $2}' | while read m; do echo -n \"$m: \"; curl -s -o /dev/null -w '%{http_code}' --max-time 5 \"$m\" 2>/dev/null; echo; done | head -5"],
    "panel_ver" => ["面板版本与仓库", "head -30 Cargo.toml 2>/dev/null | grep -E '^name|^version' | head -2"],
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
puts "batch5: created #{P.size} plugins, tools=#{P.sum { |_, m| m[1].size }}"