# vPanel · One Server, One Entry, Total Control

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![GitHub](https://img.shields.io/badge/GitHub-vexify--org%2FvPanel-181717.svg?logo=github)](https://github.com/vexify-org/vPanel)

> 用纯 Rust 从零手写的一台「服务器控制台」——**常驻内存约 1MB**，重载下不到 2MB，硬性预算 10MB。
>
> 极简是设计哲学，边界由扩展决定：进程、服务、安全、任务、应用商店、**AI 直连** —— 都在同一个屏幕里。
>
> **vPanel** 是一个单二进制、YAML 驱动、零依赖的面板：没有数据库、没有缓存、没有重型运行时。丢上去，跑起来，它就是你的。

---

## 目录

- [它到底是什么？](#它到底是什么)
- [核心亮点](#核心亮点)
- [界面预览](#界面预览)
- [快速开始](#快速开始)
  - [直接下载（推荐 · 零依赖）](#直接下载)
  - [静态编译 · 全平台](#静态编译)
  - [Alpine（Linux）](#alpine-linux)
  - [从源码构建](#从源码构建)
  - [Docker / systemd / 开机自启](#服务部署)
- [命令行（CLI）](#命令行cli)
- [功能详解](#功能详解)
- [AI 工具（MCP）](#ai-工具mcp)
- [插件系统（自定义 DSL + 微脚本语言）](#插件系统)
- [IotaPanel 兼容运行时（独立进程插件）](#iotapanel-兼容运行时)
- [配置参考（panel.yml）](#配置参考)
- [内置 HTTPS 与反向代理](#内置-https-与反向代理)
- [资源告警（SMTP 邮件通知）](#资源告警smtp-邮件通知)
- [API 参考（/api/* 与 /p/*）](#api-参考)
- [内存设计](#内存设计)
- [安全加固](#安全加固)
- [常见问题（FAQ）](#常见问题faq)
- [路线图](#路线图)
- [许可](#许可)

---

## 它到底是什么？

**vPanel** 是一门「把服务器装进浏览器」的单文件技术栈。它既不是又一个又厚又重的 LNMP 全家桶，也不是一个需要 Node/Python/数据库才能跑起来的"轻"面板。

它是：

- **一个二进制** —— 下载即用，无需 `curl | bash` 也无所谓，反正它本身就是一个独立的可执行文件，连通达一个巨型面板也不需要。
- **一套 YAML 配置** —— 每个字段都有合理默认值，空配置文件也能启动。
- **一个自带 AI 大脑的网关** —— 内建 `/mcp` 端点，Claude / Cursor / 任何 MCP 客户端都可以直接驱动它。
- **一个插件引擎** —— 一门自研的精简 DSL + 微脚本语言，一个 YAML 文件就是一个能力，20 个事件钩子跟随服务器的心跳。

典型的适用场景：

| 场景 | 你用什么 |
|------|----------|
| 低配 VPS（512MB～1GB） | 全套监控 + 面板，几乎不占资源 |
| 想要一个「能聊天的面板」 | 让 AI 通过 MCP 直接管理你的服务器 |
| 想自定义能力 | 写一个 YAML 插件，5 分钟上线 |
| 想跑独立进程插件 | IotaPanel 兼容协议，任意语言都能写 |

---

## 核心亮点

- **轻到骨头里** —— 一套手工编写的 HTTP 服务器，静态编译、低常驻内存。你的低配 VPS 几乎感觉不到它的存在。
- **开箱即用，功能齐全** —— 监控 / 进程 / 服务 / 防火墙 / 定时任务 / 应用商店 / 文件管理 / 实时日志 / Web 终端 / 数据库 / 证书 / 环境 / 备份 / 安全加固 / 站点管理，应有尽有，绝无冗余。
- **活着，并且有 AI** —— 内建 `/mcp` 端点（MCP Streamable HTTP）。任何 AI 客户端都能直接驱动面板。这不是一个仪表盘，而是一台**有大脑的服务器**。
- **可扩展，靠插件** —— 一门自研的精简 DSL + 微脚本语言。一个 YAML 文件等于一个能力，20 个事件钩子跟随服务器心脏跳动。
- **静态编译，全平台交付** —— x86_64 / ARM64 / ARMv7 三个架构的完整静态二进制，无 GLIBC 版本地狱。

---

## 界面预览

> 一个典型的面板主界面会包含：顶部的系统概览卡、实时的 CPU/内存/磁盘/网络曲线、进程列表、服务列表、防火墙规则、定时任务……

由于项目强调极简与低资源占用，前端是一份紧凑的单页应用（内嵌于二进制中），常用模块：

- **概览**：系统快照 + 历史曲线
- **进程**：按内存排序，可强杀
- **服务**：`systemctl` 启停重启
- **防火墙**：`ufw` 端口放行 / 拒绝 / 开关
- **定时任务**：`crontab` 增删查
- **应用商店**：软件一键安装，支持远程目录
- **文件管理**：浏览 / 上传 / 下载 / 编辑 / 删除
- **实时日志**：浏览器端 `tail -f`
- **Web 终端**：WebSocket + PTY 原生 Shell
- **数据库**：MySQL/MariaDB 库、用户、授权、备份、恢复
- **证书**：导入 / 自签 / Let's Encrypt 签发 / 应用到站点
- **环境**：Nginx 与 PHP 运行时安装、服务管理
- **备份**：目录 + 数据库定时全量备份
- **安全**：登录保护、IP 封禁、防爆破、SSH 加固、WAF
- **站点**：Nginx 站点创建 / 启停 / 伪静态
- **告警**：资源超限 SMTP 邮件通知
- **AI 工具**：MCP 连接信息、工具自检、交互式测试台

---

## 快速开始

### 直接下载

从 [Releases](https://github.com/vexify-org/vPanel/releases) 下载对应架构的二进制（零依赖）：

| 文件 | 架构 | 说明 |
|------|------|------|
| `vpanel-linux-amd64` | x86_64 / amd64 | 绝大多数云服务器 |
| `vpanel-linux-arm64` | ARM64 / aarch64 | 树莓派 4、Ampere、Apple Silicon VPS |
| `vpanel-linux-armv7` | ARMv7 / armhf | 32 位 ARM，树莓派 2/3 等 |

```bash
chmod +x vpanel-linux-amd64
mv vpanel-linux-amd64 /usr/local/bin/vpanel
vpanel                       # 自动在当前目录查找 panel.yml / config.yml
```

验证文件完整性：

```bash
sha256sum -c SHA256SUMS
```

### 静态编译

全部产物均为 **静态链接的 musl 二进制**，不依赖目标机的 GLIBC 版本，因此：

- 在任何现代 Linux 发行版上直接运行；
- 在 Alpine Linux 上同样开箱即用；
- 彻底告别 `GLIBC_2.39 not found` 之类的依赖错误。

### Alpine Linux

静态链接 musl，Alpine 下直接可用。如需系统包方式安装，参考 release 中附带的 `.apk` 包与公钥：

```bash
# 把公钥放入 /etc/apk/keys 后，从本地包安装（按架构选择）
apk add vpanel-x86_64-1.6.1-r0.apk   # x86_64
apk add vpanel-aarch64-1.6.1-r0.apk  # aarch64
```

### 从源码构建

需要 Rust 工具链（stable 即可，2021 edition）：

```bash
cargo build --release
./target/release/vpanel
```

启用加密 SMTP（资源告警的 STARTTLS/SSL/TLS 需要）时，额外加 `--features tls`：

```bash
cargo build --release --features tls
```

交叉编译（在 x86_64 上产出 ARM 产物）：

```bash
# aarch64
CC_aarch64_unknown_linux_musl=aarch64-linux-gnu-gcc \
AR_aarch64_unknown_linux_musl=aarch64-linux-gnu-ar \
cargo build --release --target aarch64-unknown-linux-musl --features tls

# armv7
CC_armv7_unknown_linux_musleabihf=arm-linux-gnueabihf-gcc \
AR_armv7_unknown_linux_musleabihf=arm-linux-gnueabihf-ar \
cargo build --release --target armv7-unknown-linux-musleabihf --features tls
```

构建完成后，浏览器打开：

```
http://<host>:8080/
```

---

### 服务部署

#### 前台 / 后台

```bash
vpanel                    # 前台运行（调试友好）
vpanel start              # 后台运行，日志写入 vpanel.log
vpanel stop               # 停止后台进程
vpanel restart            # 重启
vpanel log                # 查看最近日志
vpanel status             # 查看状态（监听、TLS、内存、pid）
```

#### 开机自启（systemd 示例）

把 `vpanel start` 交给 systemd 前，先安装好二进制并放置 `panel.yml`：

```ini
[Unit]
Description=vPanel Panel
After=network.target

[Service]
WorkingDirectory=/opt/vpanel
ExecStart=/usr/local/bin/vpanel
Restart=always

[Install]
WantedBy=multi-user.target
```

```bash
systemctl enable --now vpanel
```

#### 建议

- 公网部署建议在前面加 HTTPS 反向代理（Nginx / Caddy），或以内置 TLS 直接终结（见 [内置 HTTPS](#内置-https-与反向代理)）。
- 数据目录默认是当前工作目录，可用环境变量 `VPVPANEL_DIR` 覆盖。

---

## 命令行（CLI）

完整子命令一览：

```
vPanel — 极简、低常驻内存的 HTTP 面板

用法:
  panel <config.yml>        指定配置文件启动（前台）
  panel                     自动在当前目录查找配置文件（前台）
  panel start [config.yml]  后台启动（输出写入 vpanel.log）
  panel stop                停止后台进程
  panel restart             重启后台进程
  panel log [-n 200]        查看最近日志
  panel status              查看当前状态
  panel backup              手动执行一次全量备份（目录 + 数据库）
  panel uninstall           停止并清理运行时文件
  panel version             显示版本
  panel help                显示本帮助
```

另有两个环境变量：

| 变量 | 默认 | 说明 |
|------|------|------|
| `VPVPANEL_DIR` | 当前工作目录 | 数据 / 运行时文件目录 |
| `TZ_OFFSET` | `28800`（+8） | 时区偏移（秒），用于任务与告警时间显示 |

---

## 功能详解

### 系统监控

- 实时 CPU / 内存 / 磁盘 / 网络曲线，采用**有界环形缓冲区**，历史数据不会无限增长。
- 系统快照：内核版本、架构、CPU 型号与核数、温度、分区详情（只读）。
- 网络连接：连接状态与端口聚合统计，支持**按端口强杀**连接。
- 磁盘占用 TOP：指定路径下最占空间的目录 / 文件排行。
- 资源 TOP：按 CPU / 内存排序的瞬时进程排行。

### 进程管理

- 基于 `/proc`，按 RSS 排序，直观看到谁在吃内存。
- 支持强杀进程（`kill pid`）。

### 服务管理

- 基于 `systemctl`，启停 / 重启任意 systemd 服务。

### 防火墙

- 基于 `ufw`：放行 / 拒绝端口，支持协议与 IP，可整体开关。

### 定时任务

- 基于 `crontab`：新增、列出、删除 5 段 cron 计划。

### 文件管理与日志

- 目录浏览、文本编辑、上传（上限 8MB）、下载、删除。
- 实时日志：浏览器端 `tail -f` 任意文件，增量拉取、有界高效。

### Web 终端

- WebSocket + PTY，原生 Shell 在浏览器里，支持自定义命令（默认 `/bin/sh`）。

### 数据库（MySQL / MariaDB）

- 管理账号通过 `mysql` / `mysqldump` 客户端连接。
- 建库 / 删库、建用户 / 删用户、授权（`grant`）。
- 单库备份 / 恢复；重置 root 密码。

### 证书（SSL）

证书统一存放在 `certs/` 目录，每个证书一个子目录：

- **导入**：粘贴 fullchain + privkey 生成证书记录。
- **自签**：快速生成本地开发 / 内网自签证书，指定域名与有效期。
- **Let's Encrypt**：通过 `acme.sh`（需已安装）在线签发。
- **应用到站点**：把证书绑定到某个站点（可附带 HTTPS 升级）。

### 环境（运行时）

- 查看前端 / PHP 运行时状态。
- 安装所需运行时，管理 `nginx` / `php-fpm` 等服务。

### 备份

- 备份根目录、每个备份源保留的版本数、定时备份 cron 均可配置。
- 手动 `panel backup` 或调用 `crontab` 定时执行（目录 + 数据库全量）。
- 页面可视化列出备份、来源、保留数量。

### 安全加固

- **登录保护**：`security.enabled` 开启后进入初始设置向导，之后所有页面与 API 均需登录。
- **失败锁定**：连续失败达到阈值锁定一段时间，防爆破。
- **会话管理**：会话有效期、「记住我」、单账号单会话（新登录自动踢掉旧会话）。
- **IP 封禁 / 解封**：按 IP 与端口封禁，查看当前封禁列表。
- **SSH 加固 / 回滚**：一键加固 SSH 并保留回滚路径。
- **WAF**：启用 / 禁用基本 Web 防护，查询防护状态。

### 站点（Nginx）

- 创建站点（域名、监听端口、是否启用 PHP 及版本）、启停、删除（可连带删除根目录）。
- 伪静态规则一键应用（rewrite）。
- Nginx 配置管理：新增反代 Site、启停、删除、reload。
- 开机自启管理。

### 反向代理

- 路径式反代：把面板自身端口上的某个路径前缀反代到任意本机 TCP 服务，例如 `{ prefix: "/app", target: "127.0.0.1:8088" }`。
- 无需额外监听线程，可直接复用内置 TLS。

---

## AI 工具（MCP）

面板暴露一个 **MCP Streamable HTTP** 端点：`POST /mcp`。

- 支持 `initialize` / `tools/list` / `tools/call`。
- 让你的 Claude / Cursor / 任意 MCP 客户端指向 `http://<host>:8080/mcp`，AI 立即获得整套管理能力：系统监控、进程管理（含强杀）、服务管理（启停重启）、防火墙规则、定时任务，以及每一个插件工具（命名 `p_<插件>_<工具>`）。
- 内置「AI 工具」页面提供连接信息、工具自检、交互式测试台。

**内置 808 个 MCP 工具**（纯函数、各自独立），叠加插件系统后总计可达 **1,244 个工具**。一整支运维军队，由 AI 指挥。

| 安全考量 | 说明 |
|----------|------|
| 登录保护 | MCP 走面板登录会话，或独立 Bearer 令牌（`security.mcp_token`）。 |
| 白名单 | 核心工具白名单过滤，默认拒绝未知 / 废弃工具。 |

---

## 插件系统

插件是一个 YAML 文件，丢进插件目录（默认 `plugins/`）即自动加载。脚本语言是一门自研微语言：**缩进分块 + 控制流**，支持条件 / 循环 / 比较、工具入参、KV 持久化、文本与数学函数库。无重型运行时；每次执行都会启动一个全新解释器，用完即销毁。

### 语言特性

- 变量（字符串 / 数字 / 布尔），赋值，算术 `+ - * / %`，字符串 `+` 拼接。
- 比较 `== != < <= > >=` 与逻辑 `and or not`。
- 控制流：`if / else`、`for i in range(n)`、`while`、`break / continue`；块以缩进界定，以 `end` 结束。
- 工具入参：`arg("id")` / `has_arg("id")`。
- KV 持久化：`kv_set("k","v")` / `kv_get("k")`，按插件命名空间隔离，自动持久化、重启不丢失。
- 内置函数：`cmd` / `fetch` / `ret` / `log` / `env` / `var`；文本 `len/substr/split/atoi/itoa/upper/lower/trim`；数学 `min/max/round/ceil/floor`；结构化 `json("...")`。

### 管理能力

- 启 / 禁用：`POST /api/plugin/<name>/enable` / `disable`，状态持久化。
- 生命周期：`POST /api/plugin/<name>/uninstall` 移除清单并热重载。
- 在线安装 / 更新：`GET /api/plugin/store` 列出目录；`POST /api/plugin/store/install` 下载并热重载。
- 自定义表单 / 页面：在工具上声明 `params`，前端自动渲染表单，MCP 自动生成 `inputSchema`。
- 工具同时注入 UI 与 MCP，支持周期任务、20 个事件钩子。

### 案例 `plugins/demo.yml`

```yaml
name: demo
version: 1.1.0
tools:
  - id: greet                 # /api/plugin/demo/greet 与 MCP p_demo_greet
    desc: Say hello with arguments
    params:
      - id: name
        name: Name
        type: string
        required: true
      - id: times
        name: Times
        type: number
        default: "1"
    script: |
      n = arg("times")
      if n == "" || n == "0"
        n = 1
      end
      out = ""
      for i in range(atoi(n))
        out = out + "Hello, " + arg("name") + " (#" + itoa(i + 1) + ")"
      end
      ret("Greeting: " + out)
  - id: counter                 # KV 持久化示例
    desc: KV counter (survives restarts)
    script: |
      c = atoi(kv_get("count")) + 1
      kv_set("count", itoa(c))
      ret("Ran " + itoa(c) + " times")
tasks:                          # 内置周期任务（无需 crontab）
  - id: heartbeat
    every: 10
    script: |
      log("Heartbeat " + cmd("date \"+%H:%M:%S\""))
hooks:                          # 20 个事件钩子之一
  - event: on_init
    script: |
      log("Plugin loaded")
```

---

## IotaPanel 兼容运行时

vPanel 额外兼容 **IotaPanel 的独立进程插件协议**：

- 插件是一个独立同级的**进程**，可用任意语言编写（Go、Rust、Python、Node.js、Shell…），崩溃相互隔离。
- 目录结构：`plugins/<name>/manifest.yaml` + `bin/<command>`。
- 面板分配端口并注入 `PLUGIN_PORT` / `PLUGIN_NAME` 等环境变量，网关 `/p/<name>/*` 反向代理到该进程。
- 按需冷启动，空闲自动退出释放内存（`ida.idle_secs` 可配）。
- 支持从 URL / GitHub Release 安装插件包（可选 SHA256 校验），或手动放入目录即装即用。

在线能力：`GET /api/iota`（列表）、`/api/iota/start|stop|restart|keepalive|uninstall`、`/api/iota/install_url`（URL + sha256）、`/api/iota/log`（日志尾部）。

---

## 配置参考

每个字段都有合理默认值。空配置文件也能启动 —— 面板始终保证可用性第一。完整示例见 [panel.yml](./panel.yml)。

```yaml
server:
  bind: "0.0.0.0"    # 监听地址
  port: 8080          # 监听端口
  workers: 1          # 固定工作线程数（约束内存）
  backlog: 1024       # 连接队列上限，满则拒绝
  tls:                # 内置 HTTPS（见下文）
    enabled: false
    cert_file: ""     # 已有证书 PEM；留空自动生成自签
    key_file: ""
    host: "vpanel.local"  # 自签证书的 CN / SAN
  proxies: []         # 路径式反向代理，如 { prefix: "/app", target: "127.0.0.1:8088" }

panel:
  title: "vPanel"
  subtitle: "极简 · 低内存 HTTP 面板"
  accent: "#2563eb"   # 主题强调色
  theme: "light"      # light | dark

shell:                # Web 终端
  enabled: true
  cmd: "/bin/sh"      # 或 /bin/bash
  args: []
  columns: 100
  rows: 30

download:             # 应用商店
  accel: "https://g.z321.cc.cd/"       # 全局下载加速前缀
  store: "vexify-org/vp-store@main"    # 目录仓库 owner/repo@branch
  docker_dir: "/docker"                # kind:docker 的解压目标

plugins:
  dir: "plugins"      # 插件目录，*.yml 自动加载

security:             # 登录安全
  enabled: false
  password: ""        # 明文，仅首次设置用；设完存哈希，可留空走向导
  mcp_token: ""       # MCP 独立 Bearer 令牌（可选）
  max_failures: 5
  lock_minutes: 5
  session_hours: 24
  remember_days: 30
  single_session: true
  trust_proxy: false  # 位于受信 HTTPS 反代后置真

database:             # 数据库管理
  user: "root"
  password: ""
  bin: "mysql"
  dump: "mysqldump"
  backup_dir: "<panel_dir>/db-backup"

backup:               # 备份
  dir: "<panel_dir>/backup"
  keep: 5             # 每个来源保留版本数
  cron: "0 3 * * *"   # 定时备份 cron

certs:                # SSL 证书
  dir: "<panel_dir>/certs"
  le: false           # 是否启用 acme.sh Let's Encrypt

iota:                 # IotaPanel 兼容运行时
  home: "<panel_dir>/iota"
  prefix: "/p"
  port_lo: 20000
  port_hi: 21999
  idle_secs: 300      # 空闲退出秒数，0 不自动退出
```

---

## 内置 HTTPS 与反向代理

对对齐 IotaPanel 的 `https-front`、也为了省掉前置代理：

- 内置 TLS：`server.tls.enabled: true` 即可开启。
  - 提供 `cert_file` / `key_file` 则使用已有证书，
  - 否则**自动生成一次性自签证书**（立即可用，浏览器会提示证书警告）。
- 路径式反向代理：在 `server.proxies` 声明 `{ prefix, target }`，把面板端口上的某个路径前缀代理到任意本机 TCP 服务。TLS 复用上面的配置，**无需额外监听线程**。
- 当面板位于受信 HTTPS 反代之后时，设置 `security.trust_proxy: true` 以正确识别 HTTPS 与原始域名。

---

## 资源告警（SMTP 邮件通知）

想让服务器"自己开口说话"？配置一个 SMTP 服务地址，当资源超过阈值时自动发邮件通知你。

- **监控四项**：CPU 使用率、内存使用率、根分区磁盘使用率、下行带宽。
- **检测节奏**：后台低栈线程（192KB）每 12 秒检查一次，不阻塞主服务。
- **传输模式**：`plain`（明文）、`starttls`、`ssl/tls` 三种；加密路径基于 `rustls`，需以 `--features tls` 构建。
- **防抖**：冷却时间（默认 900 秒）防止告警风暴；上次发送时间持久化到配置文件，重启不丢。
- **持久化**：告警配置保存到 `alert.json`，重启后不丢失。

配置与测试接口（页面亦提供可视化入口）：

| 接口 | 说明 |
|------|------|
| `POST /api/alert/save` | 保存 SMTP 主机/端口/账号/密码/收发件人/模式与 cpu/mem/disk/net 阈值、cooldown |
| `POST /api/alert/enable` / `disable` | 开关告警 |
| `POST /api/alert/test` | 发送一封测试邮件 |
| `GET /api/alert` | 当前配置 + 实时各项资源值 |

---

## API 参考

### 查询类（GET `/api/*`）

- `/api/system` — 系统快照 + 历史曲线
- `/api/monitor?n=` — 监控曲线数据（默认 120 点）
- `/api/processes` — 进程列表
- `/api/services` — 服务列表
- `/api/firewall` — 防火墙规则
- `/api/tasks` — 定时任务
- `/api/shop` — 应用商店目录
- `/api/plugins` / `/api/plugin/kv` — 插件列表 / KV 一览
- `/api/conns` — 网络连接
- `/api/disk/top?path=&n=` — 磁盘占用 TOP
- `/api/top?n=` — 资源 TOP
- `/api/files?path=` — 目录列表
- `/api/file/read?path=` — 读取文件
- `/api/log/tail?file=&n=` / `/api/log/follow?file=&pos=` — 日志
- `/api/db/status` / `databases` / `users` / `backups` — 数据库状态
- `/api/ssl` / `/api/env` / `/api/backup` — 证书 / 环境 / 备份
- `/api/security/bans` / `hardening` / `waf` — 安全
- `/api/nginx` / `/api/website` / `/api/autostart` — 站点与自启
- `/api/iota` / `/api/iota/status?name=` — Iota 插件运行时
- `/api/proxy` — 反向代理列表

### 操作类（POST `/api/*`，表单 `application/x-www-form-urlencoded`）

- 进程：`/api/process/kill` — `pid`
- 服务：`/api/service/action` — `name`, `action=start|stop|restart`
- 防火墙：`/api/firewall/add` / `del` / `enable` / `disable`
- 定时任务：`/api/tasks/add` — `schedule`, `command`
- 连接：`/api/conn/kill` — `port`
- 文件：`/api/file/delete` / `save` / `upload(?path=)`
- 端口连接：`/api/conn/kill`
- 商店 / 插件：`/api/shop/install`(`id`)、`/api/plugin/store/install`(`id`)、`/api/plugin/<p>/<t>`、`enable`、`disable`、`uninstall`
- 告警：`/api/alert/save` / `enable` / `disable` / `test`
- 数据库：`/api/db/create_db` / `drop_db` / `create_user` / `drop_user` / `grant` / `backup` / `restore` / `reset_root`
- 证书：`/api/ssl/import` / `self_signed` / `le_issue` / `apply`
- 环境：`/api/env/install` / `service`
- 安全：`/api/security/ban` / `unban` / `brute` / `waf/enable` / `waf/disable` / `harden` / `unharden`
- 站点 / Nginx：`/api/website/create` / `toggle` / `delete` / `rewrite`、`/api/nginx/add` / `toggle` / `delete` / `reload`、`/api/autostart`
- 反向代理：`/api/proxy/add` / `del`
- 系统：`/api/system/restart`
- Iota：`/api/iota/start` / `stop` / `restart` / `keepalive` / `uninstall` / `install_url`

### 其它端点

- `POST /mcp` — MCP 端点（`initialize` / `tools/list` / `tools/call`）
- `/p/<name>/*` — Iota 插件网关反代
- `/health` → `ok`；`/metrics` → 请求 / 并发 / 内存统计

> 进程 / 服务 / 防火墙 / 定时任务等操作需要 `root` 权限，并依赖 `systemd` / `ufw` / `crontab` 可用。

---

## 内存设计

- 手工编写的 HTTP 服务器：每个连接服务完立即关闭，不残留缓冲区。
- 固定线程池 + 有界队列：高并发下连接在内核层背压或丢弃，内存不随并发增长。
- 系统命令（`systemctl` / `ufw` / `crontab` / `df`）作为一次性子进程运行，请求结束即释放。
- 监控曲线用定长环形缓冲区；系统快照与进程列表按需读取、立即释放。
- 多线程堆分配器 arena 压缩到 1 个（`MALLOC_ARENA_MAX=1`），进一步压低常驻内存。

基准：空闲约 **0.8MB**；300 并发请求（开启应用商店与 MCP）后约 0.9MB —— 仍远在 10MB 预算之内。

---

## 安全加固

- 登录保护与初始设置向导（`security.enabled`）。
- 失败锁定 + 单账号单会话 + 会话 / 记住我有效期。
- MCP 可独立 Bearer 令牌，并做核心工具白名单过滤。
- IP 封禁、SSH 加固与回滚、WAF 开关。

> ⚠️ **公网部署务必前置 HTTPS**：默认以明文 HTTP 监听 `:8080`，不经加密代理直接暴露公网时，管理员口令与会话可能被网络嗅探。建议用 Nginx / Caddy 反代终结 TLS，或开启内置 TLS。

---

## 常见问题（FAQ）

**Q：为什么不用数据库 / Redis / 运行时？**
A：极简第一。所有状态（登录、KV、告警、配置）都以文本文件持久化，重启即恢复，无需任何附加服务。

**Q：提示 `./vpanel: GLIBC_2.39 not found`？**
A：使用 release 页的 **静态 musl** 二进制（`vpanel-linux-*`）即可，它们不依赖目标机 GLIBC 版本。

**Q：加密 SMTP（STARTTLS/SSL）显示"当前构建未启用"？**
A：需要用 `--features tls` 构建（参考 [从源码构建](#从源码构建)）。release 二进制已默认启用。

**Q：哪些功能需要 root？**
A：进程 / 服务 / 防火墙 / 定时任务等系统级操作需要 root，并依赖 `systemd` / `ufw` / `crontab`。

**Q：如何让 AI 管理我的服务器？**
A：把 MCP 客户端指向 `http://<host>:8080/mcp`，详见 [AI 工具（MCP）](#ai-工具mcp)。

**Q：页面按钮点了提示"缺少 name/action/ID"？**
A：请升级到 1.6.x（已修复表单 `Content-Type` 与参数解析）。操作类接口请用 `application/x-www-form-urlencoded` 编码。

---

## 路线图

- 完善内置 TLS/ACME 自动化签发。
- 插件市场更丰富的官方包与一键模板。
- 更多运行时（Node/Python 等）的一键环境管理。
- 图形化的备份恢复 / 站群管理增强。
- 更多安全基线（fail2ban 集成、自定义 WAF 规则）。

---

## 许可

[Apache-2.0](./LICENSE)

---

**Powered By Vexify.**