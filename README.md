# vPanel · 清亮低内存 HTTP 面板

一个用 Rust 手写、**常驻内存约 1.5MB、高并发峰值 < 2MB、预算上限 10MB** 的极简服务器管理面板。不依赖 tokio/hyper，使用标准库 + 固定线程池 + 有界队列，仅在内核背压下丢弃连接，内存恒定有界。

- 单二进制，YAML 配置，零外部资源依赖。
- 浏览器内多标签管理控制台：概览 / 进程 / 服务 / 安全 / 定时任务 + Web 终端。
- 代码量小、可读性强，适合在低配 VPS 上常驻。

## 快速开始

```bash
# 构建
cargo build --release

# 运行（自动在当前目录查找 panel.yml / panel.yaml / config.yml / config.yaml）
./target/release/panel

# 或显式指定配置
./target/release/panel /path/to/panel.yml
```

启动后访问 `http://<host>:8080/`。

## 功能

| 模块 | 说明 |
|------|------|
| 系统监控 | CPU / 内存 / 磁盘 / 网络实时曲线（60 点有界环形缓冲）+ 负载 |
| 进程管理 | 读取 `/proc`，按 RSS 排序，可结束进程 |
| 服务管理 | 基于 `systemctl` 的服务的 start / stop / restart |
| 防火墙 | 基于 `ufw` 的端口放行 / 删除 |
| 定时任务 | 基于 `crontab` 的增删查 |
| Web 终端 | WebSocket + PTY，浏览器内本地 Shell 控制（按需连接） |
| 心跳 | `/health` 返回 `ok`，`/metrics` 返回请求/并发/内存统计 |

> 注意：进程 / 服务 / 防火墙 / 定时任务等操作需要相应的 `root` 权限及 `systemd` / `ufw` 环境。

## 配置

全部字段均有默认值，最小化甚至空配置文件均可启动。示例见 [panel.yml](./panel.yml)。

```yaml
server:
  bind: "0.0.0.0"   # 监听地址
  port: 8080         # 监听端口
  workers: 4         # 固定工作线程数（内存受此约束不会暴涨）
  backlog: 1024      # 连接队列上限，满则拒绝，保证内存有界

panel:
  title: "vPanel"
  subtitle: "极简 · 低内存 HTTP 面板"
  accent: "#2563eb"  # 主题强调色
  theme: "light"     # light | dark

shell:               # Web 终端
  enabled: true
  cmd: "/bin/sh"     # 可改为 /bin/bash
  args: []
  columns: 100
  rows: 30
```

## 内存设计

- 手写 HTTP 服务器：每请求处理完即关闭连接，不保存长连接缓冲。
- 固定线程池 + 有界队列：高并发时新连接在内核背压或直接丢弃，内存不随并发膨胀。
- 系统命令（systemctl / ufw / crontab / df）按需执行一次性子进程，随请求结束即释放。
- 监控曲线用定长环形缓冲；系统快照与进程列表按请求现场读取后立即释放。

实测：常驻约 1.5MB；200 并发请求后约 1.6MB。

## 接口（/api/*）

- GET `/api/system` — 系统快照 + 曲线
- GET `/api/processes` — 进程列表
- GET `/api/services` — 服务列表
- GET `/api/firewall` — 防火墙规则
- GET `/api/tasks` — 定时任务
- POST `/api/process/kill` — `pid`
- POST `/api/service/action` — `name`, `action=start|stop|restart`
- POST `/api/firewall/add` / `/api/firewall/del` — `port`
- POST `/api/tasks/add` — `schedule`(5 段 cron), `command`

## License

[MIT](./LICENSE)