# vPanel · 一台服务器，一个入口，掌控全局

> 用 Rust 从零锻造的轻量级服务器控制台 —— 常驻内存约 **0.4MB**，峰值约 **3MB**，预算上限 **10MB**。
> 极致克制，却足以纳百川：进程、服务、安全、任务、商店、AI，尽在一屏。

**vPanel** 是一枚单二进制、YAML 驱动、零外部依赖的面板。它不依赖数据库、不依赖缓存、不依赖任何重型运行时，起手即用，落地即静。

---

## 为什么是 vPanel

- **轻，轻到极致**：一个静态链接的二进制，常驻约 0.4MB，低配 VPS 也能举重若轻。
- **全，全在天生**：监控 / 进程 / 服务 / 防火墙 / 定时任务 / 软件商店 / 文件管理 / 实时日志 / Web 终端，开箱即得。
- **活，活在 AI**：内置 `/mcp` 端点（MCP Streamable HTTP），让任何 AI 客户端直接驱动面板 —— 这不是面板，是一个被赋予手脚的操作系统。
- **扩，扩于插件**：自研极简 DSL 与微脚本语言，一个 YAML 即一个能力，20 个事件钩子随心跳起舞。

---

## 快速开始

### 从预编译产物启动（零依赖，推荐）

前往 [Releases](https://github.com/vexify-org/vPanel/releases) 下载对应架构：`x86` / `x64` / `arm` / `arm64`。

```bash
chmod +x vpanel-<arch>
./vpanel-<arch>          # 自动查找当前目录 panel.yml / config.yml
```

校验完整性：

```bash
sha256sum -c SHA256SUMS
```

### Alpine 安装（apk）

静态 musl 编译，Alpine 开箱即用。发布产物含已签名 APK：

```bash
# 将公钥放入 /etc/apk/keys 后，直接从本地包安装
apk add vpanel-aarch64-1.5.0-r0.apk   # 按架构选择
```

### 从源码构建

```bash
cargo build --release
./target/release/vpanel
```

启动后访问 `http://<host>:8080/`。

---

## 功能总览

| 模块 | 说明 |
|------|------|
| 系统监控 | CPU / 内存 / 磁盘 / 网络实时曲线 + 负载（有界环形缓冲） |
| 系统信息 | OS / 内核 / 架构 / CPU / 温度 / 分区明细（只读） |
| 网络连接 | 连接状态与端口聚合，可终止监听端口进程 |
| 实时日志 | 浏览器内 tail -f 任意文件，增量拉取、有界高效 |
| 文件管理 | 目录浏览 / 文本编辑 / 上传 / 下载 / 删除 |
| 进程管理 | 读取 `/proc`，按 RSS 排序，可结束进程 |
| 服务管理 | 基于 `systemctl` 的 start / stop / restart |
| 防火墙 | 基于 `ufw` 的端口放行 / 删除 |
| 定时任务 | 基于 `crontab` 的增删查 |
| 软件商店 | 一键装常用软件，清单可远程实时拉取、失败回退内置清单 |
| 插件 | 极简 DSL + 自研微脚本语言，注入前端与 MCP、周期任务、事件钩子 |
| AI 工具 | `/mcp` 端点，AI 客户端接线即用（含插件工具） |
| Web 终端 | WebSocket + PTY，浏览器内本地 Shell |
| 心跳 | `/health` 返回 `ok`，`/metrics` 返回请求 / 并发 / 内存统计 |

> 进程 / 服务 / 防火墙 / 定时任务等操作需要对应 `root` 权限及 `systemd` / `ufw` 环境。

---

## AI 工具（MCP）

面板内置基于 **MCP Streamable HTTP** 的端点：`POST /mcp`。

- 支持 `initialize`、`tools/list`、`tools/call`。
- 在 Claude / Cursor 等客户端添加 MCP 服务器并指向 `http://<host>:8080/mcp`，AI 即可调用系统监控、进程、服务、防火墙、定时任务，以及全部插件工具（形如 `p_<插件>_<工具>`）。
- 前端「AI 工具」页提供连接地址、工具自检与调用测试。

内置 **93 个** MCP 工具（纯函数、各自独立），叠加插件系统后还能按需注入自定义工具 —— 一支由 AI 直接调遣的运维军团。

---

## 插件（极简 DSL + 微脚本语言）

插件以 YAML 描述，放入插件目录（默认 `plugins/`）即自动加载。脚本为自研微语言：**缩进块 + 控制流**，支持条件 / 循环 / 比较、工具入参、KV 持久化与文本 / 数学函数库；不引入重型运行时，每次执行新建解释器、跑完即释放。

**语言能力**

- 变量（字符串 / 数字 / 布尔）、赋值、算术 `+ - * / %`、字符串 `+` 拼接
- 比较 `== != < <= > >=` 与逻辑 `and or not`
- 控制流：`if / else`、`for i in range(n)`、`while`、`break / continue`；块以缩进界定，`end` 为显式终止符
- 工具入参：`arg("id")` / `has_arg("id")`
- KV 持久化：`kv_set("k","v")` / `kv_get("k")`，按插件命名空间隔离、自动落盘、跨重启保留
- 内置函数：`cmd` / `fetch` / `ret` / `log` / `env` / `var`，文本 `len/substr/split/atoi/itoa/upper/lower/trim`，数学 `min/max/round/ceil/floor`，结构化 `json("...")`

**管理能力**

- 启用 / 禁用开关：`POST /api/plugin/<名>/enable` / `disable`，状态持久化
- 生命周期：`POST /api/plugin/<名>/uninstall` 删除清单并热重载
- 在线安装 / 更新：`GET /api/plugin/store` 拉清单；`POST /api/plugin/store/install` 下载并热重载
- 自定义表单 / 页面：工具声明 `params` 后自动渲染参数表单，MCP 自动生成 `inputSchema`
- 工具注入前端与 MCP、自带周期任务、20 个事件钩子

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

---

## 配置

全部字段均有默认值，最小化甚至空配置文件即可启动。示例见 [panel.yml](./panel.yml)。

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

---

## 内存设计

- 手写 HTTP 服务器：每请求处理完即断开，不驻留长连接缓冲。
- 固定线程池 + 有界队列：高并发下连接在内核背压或直接丢弃，内存不随并发膨胀。
- 系统命令（`systemctl` / `ufw` / `crontab` / `df`）按需执行一次性子进程，随请求结束即释放。
- 监控曲线用定长环形缓冲；系统快照与进程列表现场读取、立即释放。

实测：常驻约 0.4MB；峰值约 3MB（含软件商店 / MCP 仍远低于 10MB 预算）。

---

## 接口（/api/*）

- GET `/api/system` — 系统快照 + 曲线
- GET `/api/processes` — 进程列表
- GET `/api/services` — 服务列表
- GET `/api/firewall` — 防火墙规则
- GET `/api/tasks` — 定时任务
- GET `/api/shop` — 软件商店清单
- POST `/api/process/kill` — `pid`
- POST `/api/service/action` — `name`, `action=start|stop|restart`
- POST `/api/firewall/add` / `/api/firewall/del` — `port`
- POST `/api/tasks/add` — `schedule`(5 段 cron), `command`
- POST `/api/shop/install` — `id`

另：`POST /mcp` — MCP 端点（`initialize` / `tools/list` / `tools/call`）。

---

## License

[Apache-2.0](./LICENSE)

---

**Powered By Vexify.**