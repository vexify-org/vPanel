# vPanel · 轻量低内存 HTTP 面板

一个用 Rust 手写、**常驻内存约 1MB、高并发峰值 < 2MB、预算上限 10MB** 的极简服务器管理面板。不依赖 tokio/hyper，使用标准库 + 固定线程池 + 有界队列，仅在内核背压下丢弃连接，内存恒定有界。
感谢@vexify-root@li63050a的贡献

- 单二进制，YAML 配置，零外部资源依赖。
- 浏览器内多标签管理控制台：概览 / 进程 / 服务 / 安全 / 定时任务 / 软件商店 / AI 工具 + Web 终端。
- 内置 **MCP 端点**（`/mcp`），AI 客户端可直接接线调用面板能力。
- 代码量小、可读性强，适合在低配 VPS 上常驻。

## 快速开始

```bash
# 构建
cargo build --release

# 运行（自动在当前目录查找 panel.yml / panel.yaml / config.yml / config.yaml）
./target/release/vpanel

# 或显式指定配置
./target/release/vpanel /path/to/panel.yml
```

启动后访问 `http://<host>:8080/`。

## 功能

| 模块 | 说明 |
|------|------|
| 系统监控 | CPU / 内存 / 磁盘 / 网络实时曲线（60 点有界环形缓冲）+ 负载 |
| 系统信息 | OS / 内核 / 架构 / CPU 型号核数 / 温度 / 分区明细（只读） |
| 网络连接 | `ss/netstat` 连接状态与本地端口聚合，可终止监听端口进程 |
| 实时日志 | 浏览器内 tail -f 任意文件，每 2s 增量拉取（有界，不轮询全量） |
| 文件管理 | 目录浏览 / 文本查看编辑 / 上传 / 下载 / 删除（按需读取） |
| 进程管理 | 读取 `/proc`，按 RSS 排序，可结束进程 |
| 服务管理 | 基于 `systemctl` 的服务的 start / stop / restart |
| 防火墙 | 基于 `ufw` 的端口放行 / 删除 |
| 定时任务 | 基于 `crontab` 的增删查 |
| 软件商店 | 一键装常用软件，下载统一走加速前缀；清单可从远程仓库（`vp-store`）实时拉取，失败回退内置清单 |
| 插件 | 极简 DSL（YAML）+ 自研微脚本语言；工具注入前端与 MCP、自带定时任务、20 个事件钩子 |
| AI 工具 | 内置 `/mcp` 端点（MCP Streamable HTTP），AI 客户端可接线调用上述能力（含插件工具） |
| Web 终端 | WebSocket + PTY，浏览器内本地 Shell 控制（按需连接） |
| 心跳 | `/health` 返回 `ok`，`/metrics` 返回请求/并发/内存统计 |

> 注意：进程 / 服务 / 防火墙 / 定时任务等操作需要相应的 `root` 权限及 `systemd` / `ufw` 环境。

### AI 工具（MCP）

面板内置一个基于 MCP（Model Context Protocol）Streamable HTTP 的端点：`POST /mcp`。

- 支持 `initialize`、`tools/list`、`tools/call`。
- 在 Claude / Cursor 等客户端添加 MCP 服务器并指向 `http://<host>:8080/mcp`，AI 即可调用：系统监控、进程（含结束）、服务（启停重启）、防火墙端口、定时任务，以及所有插件工具（工具名形如 `p_<插件>_<工具>`）。
- 前端「AI 工具」页提供连接地址、工具自检与工具调用测试。

### 插件（极简 DSL + 微脚本语言）

插件用 YAML 描述，放到插件目录（默认 `plugins/`）即自动加载。脚本是自研微语言：**缩进块 + 控制流**，支持条件/循环/比较、工具入参、键值（KV）持久化与文本/数学函数库，完全不引入重型运行时，每次执行新建解释器、跑完即释放。

**语言能力**

- 变量（字符串 / 数字 / 布尔）、赋值、算术 `+ - * / %`、字符串 `+` 拼接
- 比较 `== != < <= > >=` 与逻辑 `and or not`
- 控制流：`if / else`、`for i in range(n)`、`while`、`break / continue`；块以缩进界定，`end` 为显式终止符
- 工具入参：`arg("id")` / `has_arg("id")`（前端表单 / MCP 传入）
- KV 持久化：`kv_set("k","v")` / `kv_get("k")`，按插件命名空间隔离、自动落盘，跨重启保留
- 内置函数：`cmd` / `fetch` / `ret` / `log` / `env` / `var`，文本 `len/substr/split/atoi/itoa/upper/lower/trim`，数学 `min/max/round/ceil/floor`，结构化 `json("...")`

**管理能力**

- **启用 / 禁用开关**：`POST /api/plugin/<名>/enable` / `disable`，状态持久化，禁用后工具/任务/钩子不再触发
- **生命周期**：卸载 `POST /api/plugin/<名>/uninstall` 删除清单文件并热重载，内存随即释放
- **在线安装 / 更新**：`GET /api/plugin/store` 从 `vp-store` 拉插件清单；`POST /api/plugin/store/install`（`id`）下载到插件目录并热重载，等于安装或升级
- **自定义表单 / 页面**：工具声明 `params`（name/type/required/default/options）后，前端自动渲染参数表单，MCP 自动生成 `inputSchema`
- 工具注入前端与 MCP（MCP 工具名形如 `p_<插件>_<工具>`）、自带周期任务、20 个事件钩子

示例插件 `plugins/demo.yml`：

```yaml
name: demo
version: 1.1.0
tools:
  - id: greet                 # /api/plugin/demo/greet 与 MCP 的 p_demo_greet
    desc: 用入参打招呼
    params:                     # 前端据此渲染表单，MCP 据此生成 inputSchema
      - id: name
        name: 称呼
        type: string
        required: true
      - id: times
        name: 次数
        type: number
        default: "1"
    script: |
      n = arg("times")
      if n == "" || n == "0"
        n = 1
      end
      out = ""
      for i in range(atoi(n))
        out = out + "你好，" + arg("name") + "（第" + itoa(i + 1) + "次）"
      end
      ret("问候: " + out)
  - id: counter                 # KV 持久化
    desc: KV 计数器（重启不丢）
    script: |
      c = atoi(kv_get("count")) + 1
      kv_set("count", itoa(c))
      ret("累计执行 " + itoa(c) + " 次")
tasks:                          # 面板自带周期任务（不依赖 crontab）
  - id: heartbeat
    every: 10
    script: |
      log("心跳 " + cmd("date \"+%H:%M:%S\""))
hooks:                          # 20 个事件钩子之一
  - event: on_init
    script: |
      log("插件已加载")
```

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

download:              # 软件商店
  accel: "https://g.z321.cc.cd/"     # 全局下载加速前缀
  store: "vexify-org/vp-store@main"  # 清单仓库（owner/repo@branch）

plugins:
  dir: "plugins"        # 插件目录，放 *.yml 即自动加载
```

## 内存设计

- 手写 HTTP 服务器：每请求处理完即关闭连接，不保存长连接缓冲。
- 固定线程池 + 有界队列：高并发时新连接在内核背压或直接丢弃，内存不随并发膨胀。
- 系统命令（systemctl / ufw / crontab / df）按需执行一次性子进程，随请求结束即释放。
- 监控曲线用定长环形缓冲；系统快照与进程列表按请求现场读取后立即释放。

实测：常驻约 0.8MB；300 并发请求后约 0.9MB（含软件商店 / MCP 后仍远低于 10MB 预算）。

## 接口（/api/*）

- GET `/api/system` — 系统快照 + 曲线
- GET `/api/processes` — 进程列表
- GET `/api/services` — 服务列表
- GET `/api/firewall` — 防火墙规则
- GET `/api/tasks` — 定时任务
- GET `/api/shop` — 软件商店清单（含来源模式、加速前缀）
- POST `/api/process/kill` — `pid`
- POST `/api/service/action` — `name`, `action=start|stop|restart`
- POST `/api/firewall/add` / `/api/firewall/del` — `port`
- POST `/api/tasks/add` — `schedule`(5 段 cron), `command`
- POST `/api/shop/install` — `id`（软件 ID）

另：`POST /mcp` — MCP 端点（`initialize` / `tools/list` / `tools/call`）。

## License

[Apache-2.0](./LICENSE)
