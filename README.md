# vPanel · One Server, One Entry, Total Control

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![GitHub](https://img.shields.io/badge/GitHub-vexify--org%2FvPanel-181717.svg?logo=github)](https://github.com/vexify-org/vPanel)

> 用纯 Rust 从零手写的一台「服务器控制台」——**常驻内存约 0.4MB，峰值 ≈3MB**，硬性预算 10MB。
>
> 极简是设计哲学，边界由扩展决定：进程、服务、安全、任务、应用商店、**AI 直连** —— 都在同一个屏幕里。
>
> **vPanel** 是一个单二进制、YAML 驱动、零依赖的面板：没有数据库、没有缓存、没有重型运行时。丢上去，跑起来，它就是你的。

---

## 目录

- [版本历史](#版本历史)
- [它到底是什么？](#它到底是什么)
- [与其他面板的对比](#与其他面板的对比)
- [核心亮点](#核心亮点)
- [架构总览](#架构总览)
- [界面预览](#界面预览)
- [快速开始](#快速开始)
  - [直接下载（推荐 · 零依赖）](#直接下载)
  - [静态编译原理](#静态编译原理)
  - [Alpine（Linux）](#alpine-linux)
  - [从源码构建](#从源码构建)
  - [交叉编译（全平台）](#交叉编译全平台)
  - [服务部署与开机自启](#服务部署与开机自启)
  - [升级与卸载](#升级与卸载)
- [命令行（CLI）](#命令行cli)
  - [前台与后台](#前台与后台)
  - [子命令速查](#子命令速查)
  - [环境变量](#环境变量)
  - [退出码约定](#退出码约定)
- [功能详解](#功能详解)
  - [系统监控](#系统监控)
  - [进程管理](#进程管理)
  - [服务管理](#服务管理)
  - [防火墙](#防火墙)
  - [定时任务](#定时任务)
  - [文件管理与日志](#文件管理与日志)
  - [Web 终端](#web-终端)
  - [数据库（MySQL / MariaDB）](#数据库mysql--mariadb)
  - [证书（SSL）](#证书ssl)
  - [环境（运行时）](#环境运行时)
  - [备份](#备份)
  - [安全加固](#安全加固)
  - [站点（Nginx）](#站点nginx)
  - [软件商店](#软件商店)
  - [反向代理](#反向代理)
- [内置 HTTPS](#内置-https)
- [资源告警（SMTP 邮件通知）](#资源告警smtp-邮件通知)
- [AI 工具（MCP）](#ai-工具mcp)
- [插件系统](#插件系统)
  - [插件加载与生命周期](#插件加载与生命周期)
  - [语法完整参考](#语法完整参考)
  - [内置函数总表](#内置函数总表)
  - [文本与数学函数](#文本与数学函数)
  - [列表与迭代函数](#列表与迭代函数)
  - [文件操作函数](#文件操作函数)
  - [系统与网络函数](#系统与网络函数)
  - [字符串增强函数](#字符串增强函数)
  - [JSON 与格式化函数](#json-与格式化函数)
  - [事件钩子（20 个）](#事件钩子20-个)
  - [插件完整示例](#插件完整示例)
  - [编写插件的实战建议](#编写插件的实战建议)
- [IotaPanel 兼容运行时](#iotapanel-兼容运行时)
- [配置参考（panel.yml）](#配置参考panelyml)
  - [server：监听与服务端](#server监听与服务端)
  - [panel：界面](#panel界面)
  - [shell：Web 终端](#shellweb-终端)
  - [download：软件商店](#download软件商店)
  - [plugins：插件目录](#plugins插件目录)
  - [security：登录安全](#security登录安全)
  - [database：数据库管理](#database数据库管理)
  - [backup：备份](#backup备份)
  - [certs：证书存储](#certs证书存储)
  - [iota：IotaPanel 兼容运行时](#iota-iotapanel-兼容运行时)
- [API 参考](#api-参考)
  - [约定](#约定)
  - [系统与监控](#系统与监控)
  - [进程与连接](#进程与连接)
  - [服务与定时任务](#服务与定时任务)
  - [防火墙](#防火墙)
  - [文件与日志](#文件与日志)
  - [软件商店与插件](#软件商店与插件)
  - [数据库](#数据库)
  - [证书与环境](#证书与环境)
  - [备份与安全](#备份与安全)
  - [站点与 Nginx](#站点与-nginx)
  - [反向代理与告警](#反向代理与告警)
  - [Iota 运行时](#iota-运行时)
  - [MCP 与健康检查](#mcp-与健康检查)
- [内存设计](#内存设计)
- [性能与压测](#性能与压测)
- [安全最佳实践](#安全最佳实践)
- [常见问题（FAQ）](#常见问题faq)
- [排障指南](#排障指南)
- [贡献指南](#贡献指南)
- [路线图](#路线图)
- [许可](#许可)

---

# 版本历史

| 版本 | 时间 | 关键变更 |
|------|------|----------|
| v1.6.1 | 2026-08 | 资源告警（SMTP 三种传输模式，支持加密）；重写数据库/环境/证书/备份/安全页面；表单 `Content-Type` 修复；MCP 白名单与冗余清理；静态编译全平台交付 |
| v1.5.0 | 更早 | HTTP 服务 YML 配置内存优化；插件系统完善 |
| v1.x | 起 | 从极简监控面板逐步扩展为完整控制台 |

> 发布产物均为 **静态链接的 musl 二进制**，彻底告别 `GLIBC_2.39 not found`。

---

# 它到底是什么？

**vPanel** 是一门「把服务器装进浏览器」的单文件技术栈。它既不是又一个又厚又重的 LNMP 全家桶，也不是一个需要 Node / Python / 数据库才能跑起来的"轻"面板。

它是：

- **一个二进制** —— 下载即用。它本身就是一个独立可执行文件，不依赖任何运行时。
- **一套 YAML 配置** —— 每个字段都有合理默认值，空配置文件也能启动。
- **一个自带 AI 大脑的网关** —— 内建 `/mcp` 端点，Claude / Cursor / 任何 MCP 客户端都可以直接驱动它。
- **一个插件引擎** —— 一门自研的精简 DSL + 微脚本语言，一个 YAML 文件就是一个能力，20 个事件钩子跟随服务器的心跳。
- **一个兼容外来协议的宿主** —— 兼容 IotaPanel 的独立进程插件协议，任意语言都能写插件。

### 它的本质

一切能力都被压缩进以下几条设计原则：

1. **常驻可用第一**：配置文件缺失、损坏，进程照常启动（回退默认）。
2. **极简单一**：无数据库、无缓存、无重型运行时，所有状态以文本文件持久化。
3. **按需执行**：系统命令（`systemctl` / `ufw` / `crontab` / `df` / `mysql`）都是一次性子进程，用完即释放。
4. **AI 原生**：MCP 不是"锦上添花"的插件，而是内置的一等公民。

### 典型适用场景

| 场景 | 你用什么 |
|------|----------|
| 低配 VPS（512MB～1GB） | 全套监控 + 面板，几乎不占资源 |
| 想要一个「能聊天的面板」 | 让 AI 通过 MCP 直接管理你的服务器 |
| 想自定义能力 | 写一个 YAML 插件，5 分钟上线 |
| 想跑独立进程插件 | IotaPanel 兼容协议，任意语言都能写 |
| 想管理站点 / 数据库 / 证书 | 内置站点、数据库、SSL、备份全套 |

---

# 与其他面板的对比

| 维度 | vPanel | 传统重量级面板 | IotaPanel |
|------|--------|----------------|-----------|
| 常驻内存 | ~0.4MB | 上百 MB | ~8MB |
| 依赖 | 无（单二进制） | 需要语言运行时 / 数据库 | 单二进制 |
| 配置 | YAML | 数据库 / 复杂 UI | 配置文件 |
| 插件 | YAML DSL + 微脚本 | 生态复杂 | 独立进程（任意语言） |
| AI 能力 | 内置 MCP | 无 / 非原生 | 较弱 |
| 网页内嵌 | 单页应用内嵌二进制 | 分离 | 原生 UI 融合 |

**vPanel 的取舍**：比传统面板轻一个数量级，同时内置了 AI 能力；通过兼容 IotaPanel 协议补上了"独立进程插件"这块拼图。

---

# 核心亮点

- **轻到骨头里** —— 手工编写的 HTTP 服务器，静态编译、低常驻内存。你的低配 VPS 几乎感觉不到它的存在。
- **开箱即用，功能齐全** —— 监控 / 进程 / 服务 / 防火墙 / 定时任务 / 应用商店 / 文件管理 / 实时日志 / Web 终端 / 数据库 / 证书 / 环境 / 备份 / 安全加固 / 站点管理，应有尽有，绝无冗余。
- **活着，并且有 AI** —— 内建 `/mcp` 端点（MCP Streamable HTTP）。任何 AI 客户端都能直接驱动面板。这不是一个仪表盘，而是一台**有大脑的服务器**。
- **可扩展，靠插件** —— 自研精简 DSL + 微脚本语言。一个 YAML 文件等于一个能力，20 个事件钩子跟随服务器心脏跳动。
- **静态编译，全平台交付** —— x86_64 / ARM64 / ARMv7 三个架构的完整静态二进制，无 GLIBC 版本地狱。
- **兼容开放** —— 兼容 IotaPanel 独立进程插件协议，生态系统不再被限定在单一语言。

---

# 架构总览

```
                         ┌──────────────────────────────────────────┐
   浏览器 / MCP 客户端     │                 vPanel                    │
   ───────────────►      │                                        │
                         │  ┌──────────┐   ┌───────────────────┐  │
                         │  │ 路由/鉴权 │──►│  内置单页应用(UI)   │  │
                         │  └────┬─────┘   └───────────────────┘  │
                         │       │ POST /mcp                       │
                         │  ┌────▼─────┐   ┌───────────────────┐  │
                         │  │  MCP 网关 │──►│  插件 DSL 解释器    │  │
                         │  └──────────┘   └───────────────────┘  │
                         │                                        │
                         │  ┌──────────┐  ┌────────────────────┐  │
                         │  │ 固定线程池 │  │  系统命令一次性子进程  │  │
                         │  │ +有界队列  │  │ systemctl/ufw/df... │  │
                         │  └──────────┘  └────────────────────┘  │
                         │                                        │
                         │  后台低栈线程：告警检测(12s) / 监控采样    │
                         └──────────────────────────────────────────┘
```

- **请求路径**：浏览器 → 路由/鉴权 → 数据查询（`/api/*` GET）或系统操作（POST）；MCP 客户端走 `/mcp` → 内置工具或插件工具。
- **插件两种形态**：
  - **DSL 插件**（一个 YAML 文件）→ 内嵌解释器执行，注入 UI 与 MCP；
  - **独立进程插件**（IotaPanel 协议）→ 面板分配端口，网关 `/p/<name>/*` 反代到子进程。
- **后台线程**：监控采样、资源告警（低栈 192KB）、插件周期任务，均不阻塞请求处理。

---

# 界面预览

> 典型界面：顶部系统概览卡 + 实时 CPU/内存/磁盘/网络曲线 + 各功能模块入口。

由于项目强调极简与低资源占用，前端是一份紧凑的单页应用（内嵌于二进制中），常用模块：

| 模块 | 说明 |
|------|------|
| 概览 | 系统快照 + 历史曲线 |
| 进程 | 按内存排序，可强杀 |
| 服务 | `systemctl` 启停重启 |
| 防火墙 | `ufw` 端口放行 / 拒绝 / 开关 |
| 定时任务 | `crontab` 增删查 |
| 应用商店 | 软件一键安装，支持远程目录 |
| 文件管理 | 浏览 / 上传 / 下载 / 编辑 / 删除 |
| 实时日志 | 浏览器端 `tail -f` |
| Web 终端 | WebSocket + PTY 原生 Shell |
| 数据库 | MySQL/MariaDB 库、用户、授权、备份、恢复 |
| 证书 | 导入 / 自签 / Let's Encrypt 签发 / 应用 |
| 环境 | Nginx 与 PHP 运行时安装、服务管理 |
| 备份 | 目录 + 数据库定时全量备份 |
| 安全 | 登录保护、IP 封禁、防爆破、SSH 加固、WAF |
| 站点 | Nginx 站点创建 / 启停 / 伪静态 |
| 告警 | 资源超限 SMTP 邮件通知 |
| AI 工具 | MCP 连接信息、工具自检、交互式测试台 |

---

# 快速开始

## 直接下载

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

启动后浏览器打开：

```text
http://<host>:8080/
```

### 一条命令直接跑

在任意空目录：

```bash
mkdir -p /opt/vpanel && cd /opt/vpanel
vpanel
```

无需创建任何配置文件 —— 面板会将默认配置直接跑起来（`http://<host>:8080`）。需要写 `panel.yml` 时的样例见 [配置参考](#配置参考panelyml)。

---

## 静态编译原理

全部 release 产物均为 **静态链接的 musl 二进制**，不依赖目标机的 GLIBC 版本，因此：

- 在任何现代 Linux 发行版上直接运行；
- 在 Alpine Linux 上同样开箱即用；
- 彻底告别 `GLIBC_2.39 not found` 之类的依赖错误。

**为什么需要静态编译？** 动态链接的二进制在 GLIBC 版本较旧 / 较新的机器上可能因符号缺失直接崩溃（例如 `version 'GLIBC_2.39' not found`）。静态链接把 libc 一并打进去，交付的产物在任何机器上行为一致。

---

## Alpine Linux

静态链接 musl，Alpine 下直接可用。如需系统包方式安装，参考 release 中附带的 `.apk` 包与公钥：

```bash
# 把公钥放入 /etc/apk/keys 后，从本地包安装（按架构选择）
apk add vpanel-x86_64-1.6.1-r0.apk   # x86_64
apk add vpanel-aarch64-1.6.1-r0.apk  # aarch64
```

---

## 从源码构建

需要 Rust 工具链（stable 即可，2021 edition）：

```bash
cargo build --release
./target/release/vpanel
```

启用加密 SMTP（资源告警的 STARTTLS / SSL / TLS 需要）时，额外加 `--features tls`：

```bash
cargo build --release --features tls
```

### 构建特性说明

| 特性 | 说明 |
|------|------|
| `tls` | 启用 `rustls` 加密栈，资源告警可走加密 SMTP（STARTTLS / SSL-PRE / TLS）；不含此特性则仅支持明文 SMTP |

> release 官方二进制默认启用 `tls` 特性。

---

## 交叉编译（全平台）

在 x86_64 主机上产出 ARM 产物。先安装对应 `gcc`：`gcc-aarch64-linux-gnu`、`gcc-arm-linux-gnueabihf`，并确保已添加 Rust 目标：

```bash
rustup target add aarch64-unknown-linux-musl armv7-unknown-linux-musleabihf
```

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

产物分别位于：

```text
target/aarch64-unknown-linux-musl/release/vpanel
target/armv7-unknown-linux-musleabihf/release/vpanel
```

> 注意：默认构建路径 `target/release` 为宿主架构（本机 gnu 或 musl）。若要产出 `x86_64` 的静态 musl 二进制，使用 `target x86_64-unknown-linux-musl` 并安装 musl-tools（`musl-gcc`）。此外，不同目标共享 `target/` 下的缓存，若交叉编译遇到 `ring` 构建缓存冲突（如 `File exists`），先 `cargo clean` 再分别构建。

---

## 服务部署与开机自启

### 前台 / 后台

```bash
vpanel                    # 前台运行（调试友好）
vpanel start              # 后台运行，日志写入 vpanel.log
vpanel stop               # 停止后台进程
vpanel restart            # 重启
vpanel log                # 查看最近日志
vpanel status             # 查看状态（监听、TLS、内存、pid）
```

### systemd 开机自启

先安装好二进制并放置 `panel.yml`，再写入单元文件：

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

启用并启动：

```bash
systemctl enable --now vpanel
```

> 若希望日志走 journald，把 `ExecStart` 换成 `ExecStart=/usr/local/bin/vpanel` 且前台运行即可（systemd 捕获 stdout/stderr）。用 `Restart=always` 保证崩溃自动拉起。

### 建议

- 公网部署建议在前面加 HTTPS 反向代理（Nginx / Caddy），或开启[内置 TLS](#内置-https)。
- 数据目录默认是当前工作目录，可用环境变量 `VPVPANEL_DIR` 覆盖。

---

## 升级与卸载

**升级**：直接替换二进制并以 `vpanel restart`（或 systemd `restart`）即可。配置与数据文件保留，无需迁移。

**卸载**：

```bash
vpanel stop          # 停止
vpanel uninstall     # 停止并清理 pid / 日志等运行时文件
```

彻底移除二进制请手动删除自身可执行文件。

---

# 命令行（CLI）

## 前台与后台

- **前台**：直接 `vpanel`（或 `vpanel <config.yml>`）—— 适合调试，Ctrl-C 退出。
- **后台**：`vpanel start [config.yml]` —— 派发子进程，输出重定向到 `vpanel.log`，记录 pid 到 `vpanel.pid`。

## 子命令速查

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

### `panel status` 输出示例

```
vPanel 1.6.1 (pkg: vpanel)
listen: http://0.0.0.0:8080
tls:    off
shell:  true /bin/sh
rss_kb: 812
auth:   disabled
pid:    12345 (running)
```

## 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `VPVPANEL_DIR` | 当前工作目录 | 数据 / 运行时文件目录（pid、日志、配置、数据均相对于此） |
| `TZ_OFFSET` | `28800`（+8） | 时区偏移（秒），用于任务与告警时间显示 |
| `MALLOC_ARENA_MAX` | `1` | 面板自身在启动时置 1，压缩多线程堆 arena，压低常驻内存 |

## 退出码约定

| 退出码 | 含义 |
|--------|------|
| `0` | 成功 |
| `1` | 失败（如无法监听、已在运行、未在运行） |
| `2` 及以上 | 一般由系统 / 子命令引发，不单独约定 |

---

# 功能详解

## 系统监控

| 能力 | 说明 |
|------|------|
| 实时曲线 | CPU / 内存 / 磁盘 / 网络，采用**有界环形缓冲区**，历史不无限增长 |
| 系统快照 | 内核版本、架构、CPU 型号与核数、温度、分区详情（只读） |
| 网络连接 | 连接状态与端口聚合统计，支持**按端口强杀**连接 |
| 磁盘占用 TOP | 指定路径下最占空间的目录 / 文件排行 |
| 资源 TOP | 按 CPU / 内存排序的瞬时进程排行 |
| 监控曲线数据 | `/api/monitor?n=` 拿最近 N 点数据 |

## 进程管理

- 基于 `/proc`，按 RSS 排序，直观看到谁在吃内存。
- 支持强杀进程（`POST /api/process/kill`，传 `pid`）。

## 服务管理

- 基于 `systemctl`，启停 / 重启任意 systemd 服务。
- 操作需要 root 与 systemd 可用。

## 防火墙

- 基于 `ufw`：
  - 放行 / 拒绝端口（`allow` / `deny`）；
  - 支持协议（默认 `tcp`）与 IP；
  - 整体开关（`ufw enable` / `disable`）。
- 操作需要 root 与 ufw 可用。

## 定时任务

- 基于 `crontab`：新增、列出、删除 5 段 cron 计划。
- 操作需要 root 与 crontab 可用。

## 文件管理与日志

| 能力 | 说明 |
|------|------|
| 目录浏览 | `/api/files?path=` |
| 读取文件 | `/api/file/read?path=` |
| 编辑保存 | `POST /api/file/save`（`application/x-www-form-urlencoded`，`path` + `data`） |
| 上传 | `POST /api/file/upload?path=`（body 为原始字节，上限 8MB） |
| 下载 / 删除 | 支持文件操作 |
| 实时日志 | `/api/log/tail`、`/api/log/follow`（增量拉取，有界高效） |

## Web 终端

- WebSocket + PTY，原生 Shell 在浏览器里。
- 支持自定义命令与参数（`shell.cmd` / `shell.args`，默认 `/bin/sh`）。

## 数据库（MySQL / MariaDB）

- 管理账号通过 `mysql` / `mysqldump` 客户端连接（与面板解耦）。
- 能力：建库 / 删库、建用户 / 删用户、授权（`grant`）、单库备份 / 恢复、重置 root 密码。
- 备份目录由 `database.backup_dir` 指定（默认 `<panel_dir>/db-backup`）。

| 接口 | 说明 |
|------|------|
| `GET /api/db/status` | 连接状态 |
| `GET /api/db/databases` | 数据库列表 |
| `GET /api/db/users` | 用户列表 |
| `GET /api/db/backups` | 备份列表 |
| `POST /api/db/create_db` / `drop_db` | 建 / 删库 |
| `POST /api/db/create_user` / `drop_user` | 建 / 删用户 |
| `POST /api/db/grant` | 授权 |
| `POST /api/db/backup` / `restore` | 备份 / 恢复 |
| `POST /api/db/reset_root` | 重置 root 密码 |

## 证书（SSL）

证书统一存放在 `certs/` 目录，每个证书一个子目录：

| 能力 | 说明 |
|------|------|
| 导入 | 粘贴 fullchain + privkey 生成证书记录 |
| 自签 | 快速生成自签证书，指定域名与有效期（默认 365 天） |
| Let's Encrypt | 通过 `acme.sh`（需已安装）在线签发，指定 webroot |
| 应用到站点 | 把证书绑定到某个站点（可附带 HTTPS 升级） |

## 环境（运行时）

- 查看前端 / PHP 运行时状态。
- 安装所需运行时，管理 `nginx` / `php-fpm` 等服务。

## 备份

- 备份根目录、每个备份源保留的版本数、定时备份 cron 均可配置。
- 手动 `panel backup`，或由 `crontab` 调用定时执行（目录 + 数据库全量）。
- 页面可视化列出备份、来源、保留数量。

## 安全加固

| 能力 | 说明 |
|------|------|
| 登录保护 | `security.enabled` 开启后进入初始设置向导，之后所有页面与 API 均需登录 |
| 失败锁定 | 连续失败达到阈值锁定一段时间，防爆破 |
| 会话管理 | 会话有效期、「记住我」、单账号单会话（新登录自动踢掉旧会话） |
| IP 封禁 | 按 IP 封禁 / 解封，查看当前封禁列表（持久化 `.vpanel-ban.json`） |
| 防爆破扫描 | 扫描 `journalctl -u sshd` 最近 5000 行，对超阈值来源 IP 自动封禁 |
| SSH 加固 / 回滚 | 一键加固（禁 root 密码登录、禁密码登录可选）并保留回滚 |
| WAF | 基于 Nginx `geo` + `limit_req` 生成防护配置，可整体开关 |

## 站点（Nginx）

| 能力 | 说明 |
|------|------|
| 创建站点 | 域名、监听端口、是否启用 PHP 及版本 |
| 启停 | 开启 / 关闭站点 |
| 删除 | 可连带删除站点根目录 |
| 伪静态 | 一键应用 rewrite 规则 |
| Nginx 配置管理 | 新增反代 Site、启停、删除、reload |
| 开机自启 | 管理 nginx 服务自启 |

## 软件商店

- 清单由一个 GitHub 仓库（默认 `vexify-org/vp-store` 的 `apps.yml`）维护，面板"一键爬取"走加速前缀实时拉取并缓存 **60 秒**。
- 远程拉取失败时回退**内置清单**，保证始终可用。
- 安装脚本是带 `{accel}` 占位符的 shell 模板，按需 `bash -c` 一次性执行，与常驻内存解耦。
- 全局下载统一走加速前缀（`download.accel`，默认 `https://g.z321.cc.cd/`）。
- 支持 `kind: docker` 的包解压到 `docker_dir`（默认 `/docker`）。

## 反向代理

- **路径式反代**：把面板自身端口上的某个路径前缀反代到任意本机 TCP 服务，如 `{ prefix: "/app", target: "127.0.0.1:8088" }`。
- 无需额外监听线程，可直接复用[内置 TLS](#内置-https)。

---

# 内置 HTTPS

对对齐 IotaPanel 的 `https-front`、也为了省掉前置代理：

- 内置 TLS：`server.tls.enabled: true` 即可开启。
  - 提供 `cert_file` / `key_file` 则使用已有证书，
  - 否则**自动生成一次性自签证书**（立即可用，浏览器会提示证书警告）。
- 自签证书的 CN / SAN 由 `server.tls.host` 指定（默认 `vpanel.local`）。
- 当面板位于受信 HTTPS 反代之后时，设置 `security.trust_proxy: true` 以正确识别 HTTPS 与原始域名。

```yaml
server:
  tls:
    enabled: true
    cert_file: ""          # 留空则自动生成自签
    key_file: ""
    host: "vpanel.local"   # 自签证书的 CN / SAN
```

---

# 资源告警（SMTP 邮件通知）

想让服务器"自己开口说话"？配置一个 SMTP 服务地址，当资源超过阈值时自动发邮件通知你。

### 监控什么

| 指标 | 说明 |
|------|------|
| CPU | 使用率百分比阈值（0 = 不监控） |
| 内存 | 使用率百分比阈值 |
| 磁盘 | 根分区使用率百分比阈值 |
| 带宽 | 下行带宽阈值（B/s） |

### 工作机制

- **检测节奏**：后台低栈线程（192KB）每 **12 秒**检查一次，不阻塞主服务。
- **传输模式**：`plain`（明文）、`starttls`（显式 STARTTLS）、`ssl`（隐式 TLS）三种；加密路径基于 `rustls`，需以 `--features tls` 构建。
- **防抖**：冷却时间（默认 900 秒）防止告警风暴；上次发送时间持久化到配置文件，重启不丢。
- **持久化**：告警配置保存到 `alert.json`，重启后不丢失。

### 配置与测试

| 接口 | 说明 |
|------|------|
| `POST /api/alert/save` | 保存 SMTP 主机/端口/账号/密码/收发件人/模式与 cpu/mem/disk/net 阈值、cooldown |
| `POST /api/alert/enable` / `disable` | 开关告警 |
| `POST /api/alert/test` | 发送一封测试邮件 |
| `GET /api/alert` | 当前配置 + 实时各项资源值 |

### SMTP 配置字段

```text
smtp_host    SMTP 服务器地址（host 或 host:port）
smtp_port    SMTP 端口（默认 587）
smtp_user    SMTP 账号
smtp_pass    SMTP 密码
from         发件人邮箱
to           收件人邮箱
mode         plain | starttls | ssl
cpu/mem/disk/net   各资源阈值（0 = 不监控）
cooldown     冷却秒数（默认 900）
```

---

# AI 工具（MCP）

面板暴露一个 **MCP Streamable HTTP** 端点：`POST /mcp`。

- 支持 `initialize` / `tools/list` / `tools/call`。
- 让你的 Claude / Cursor / 任意 MCP 客户端指向 `http://<host>:8080/mcp`，AI 立即获得整套管理能力：系统监控、进程管理（含强杀）、服务管理（启停重启）、防火墙规则、定时任务，以及每一个插件工具（命名 `p_<插件>_<工具>`）。
- 内置「AI 工具」页面提供连接信息、工具自检、交互式测试台。

### 工具规模

**内置 93 个 MCP 工具**（纯函数、各自独立），叠加插件系统后还能按需注入自定义工具。一整支运维军队，由 AI 指挥。

### MCP 安全性

| 安全考量 | 说明 |
|----------|------|
| 登录保护 | MCP 走面板登录会话，或独立 Bearer 令牌（`security.mcp_token`） |
| 白名单 | 核心工具白名单过滤，默认拒绝未知 / 废弃工具 |

### 让 AI 连上来（示例）

以 Claude Desktop / Cursor 等支持 MCP 的客户端，添加一条 MCP server：

```json
{
  "mcpServers": {
    "vpanel": {
      "url": "http://<host>:8080/mcp"
    }
  }
}
```

若开启了 `security.mcp_token`，在请求头加 `Authorization: Bearer <token>`。

---

# 插件系统

插件是一个 YAML 文件，丢进插件目录（默认 `plugins/`）即自动加载。脚本语言是一门自研微语言：**缩进分块 + 控制流**，支持条件 / 循环 / 比较、工具入参、KV 持久化、文本与数学函数库。无重型运行时；每次执行都会启动一个全新解释器，用完即销毁。

## 插件加载与生命周期

- **加载**：目录下 `*.yml` 自动加载，热加载（新增 / 修改即生效）。
- **启 / 禁用**：`POST /api/plugin/<name>/enable` / `disable`，状态持久化。
- **卸载**：`POST /api/plugin/<name>/uninstall` 移除清单并热重载。
- **在线安装 / 更新**：`GET /api/plugin/store` 列出目录；`POST /api/plugin/store/install` 下载并热重载。
- **工具注入**：工具同时注入 UI 与 MCP，支持周期任务、20 个事件钩子。

## 语法完整参考

### 变量

支持三种类型：

```text
变量（字符串 / 数字 / 布尔）、赋值、算术（+ - * / %）、字符串 + 拼接
```

```text
name = "world"
times = 3
flag = true
```

### 运算符

| 类别 | 运算符 |
|------|--------|
| 算术 | `+` `-` `*` `/` `%` |
| 拼接 | `+`（任一侧为字符串时） |
| 比较 | `==` `!=` `<` `<=` `>` `>=` |
| 逻辑 | `and`/`&&` `or`/`||` `not`/`!` |
| 取负 | `-x` |

### 控制流

```text
if / else
for i in range(n)          # 循环 n 次
for x in <列表>            # 遍历列表
while <条件>
break / continue
块以缩进界定，以 end 结束
```

```text
if n == "" || n == "0"
  n = 1
end

for i in range(3)
  log("第 " + itoa(i + 1) + " 次")
end
```

### 工具入参与 KV

```text
arg("id") / has_arg("id")        # 读取工具入参
kv_set("k","v") / kv_get("k")    # KV 持久化，重启不丢
```

注：脚本内可直接访问内置函数（见下表），无需额外 require。

## 内置函数总表

### 宿主能力（Builtin 接口）

| 函数 | 说明 |
|------|------|
| `cmd(cmd)` | 执行 shell 命令，返回输出 |
| `fetch(url, timeout)` | HTTP GET，超时秒数（默认 8s），返回响应文本 |
| `post(url, body)` | HTTP POST，返回响应文本 |
| `http_status(url)` | 探测 URL 的 HTTP 状态码（如 "200"），失败返回 "0" |
| `kv_get(k)` / `kv_set(k,v)` | KV 持久化，按插件隔离，自动保存 |
| `arg(name)` / `has_arg(name)` | 读取 / 判断工具入参 |
| `read_file(path)` | 读取文本文件，不存在返回空串 |
| `write_file(path, content)` | 覆盖写入文本文件，返回成功与否 |
| `append_file(path, content)` | 追加文本到文件 |
| `ls(path)` | 列目录：每行 `名称<tab>类型(d/f)<tab>大小` |
| `file_info(path)` | 文件信息：`大小;<是否存在>;<是否目录>` |
| `lookup_ip(host)` | 解析主机 → 第一个 IP（失败返回空） |
| `kill_pid(pid)` | 结束进程，成功返回 true |
| `sha1(path)` | 计算文件 SHA-1（小写 hex），失败返回 `-` |
| `urlenc(s)` | 将字符串 URL 编码 |

### 脚本内置函数（call 分发）

`ret` / `log` / `env` / `var` / `json` 已在上面出现，下面按类别列出。

## 文本与数学函数

| 函数 | 说明 |
|------|------|
| `len(x)` | 列表长度，字符串则返回字符数 |
| `substr(s, start, end)` | 按字符截取；`end` 为负时截到结尾 |
| `atoi(s)` | 转数值 |
| `itoa(n)` | 数值转字符串 |
| `min(a,b)` / `max(a,b)` | 取小 / 取大 |
| `round(n)` / `ceil(n)` / `floor(n)` | 四舍五入 / 上取整 / 下取整 |
| `upper(s)` / `lower(s)` / `trim(s)` | 转大写 / 转小写 / 去空白 |
| `split(s, sep)` | 按分隔符拆分，返回以 `|` 连接的字符串 |

## 列表与迭代函数

| 函数 | 说明 |
|------|------|
| `range(n)` | 返回数值用于 `for i in range(n)` |
| `lines(s)` | 按行拆分为列表 |
| `split_list(s, sep)` | 按分隔符拆分为列表 |
| `at(list, i)` | 取第 i 项 |
| `push(list, x)` | 追加元素，返回新列表 |
| `pop(list)` | 弹出末尾元素，返回该元素 |
| `join(list, sep)` | 用分隔符连接成字符串 |

## 文件操作函数

| 函数 | 说明 |
|------|------|
| `read_file(path)` | 读取文本文件 |
| `write_file(path, content)` | 覆盖写入 |
| `append_file(path, content)` | 追加写入 |
| `ls(path)` | 列目录 |
| `file_info(path)` | 文件信息 |
| `rm(path)` | 删除文件或目录 |

## 系统与网络函数

| 函数 | 说明 |
|------|------|
| `cmd(cmd)` | 执行 shell |
| `fetch(url, timeout)` | HTTP GET |
| `post(url, body)` | HTTP POST |
| `http_status(url)` | 探测状态码 |
| `lookup_ip(host)` | 解析 IP |
| `shasum(path)` | 计算 SHA-1 |
| `urlenc(s)` | URL 编码 |
| `kill(pid)` | 结束进程 |
| `sleep(ms)` | 休眠毫秒（上限 60 秒） |
| `now()` | 当前 epoch 秒 |
| `date()` | 当前时间 `%Y-%m-%d %H:%M:%S` |
| `date_fmt(fmt)` | 按格式取当前时间 |
| `strftime(fmt, epoch)` | 按格式格式化指定时间 |
| `rand(lo, hi)` | 随机数（单参时 [0, lo)） |

## 字符串增强函数

| 函数 | 说明 |
|------|------|
| `contains(s, sub)` | 是否包含 |
| `startswith(s, pre)` / `endswith(s, suf)` | 前缀 / 后缀 |
| `index(s, sub)` | 首次出现位置，未找到返回 -1 |
| `replace(s, a, b)` | 替换全部 |
| `rev(s)` | 反转字符串 |
| `count(s, sub)` | 出现次数 |
| `pad(s, width)` | 左补空格到指定宽度 |

## JSON 与格式化函数

| 函数 | 说明 |
|------|------|
| `json(s)` | 字符串转 JSON 引号形式 |
| `json_get(s, path)` | 读取 JSON 路径值 |
| `keys(json)` | 返回对象的键列表 |
| `compact(json)` | 去除 JSON 空白 |
| `fmt_bytes(n)` | 字节数 → 可读大小 |
| `fmt_dur(n)` | 秒数 → 可读时长 |

## 事件钩子（20 个）

面板内置 20 个事件钩子，插件可为任意事件挂脚本，在对应时刻自动执行：

| 事件 | 触发时机 |
|------|----------|
| `on_init` | 插件加载 |
| `on_shutdown` | 面板关闭 |
| `on_tick` | 周期心跳 |
| `on_http_request` | 收到 HTTP 请求 |
| `on_snapshot` | 系统快照刷新 |
| `on_process_list` | 进程列表刷新 |
| `on_service_start` | 服务启动 |
| `on_service_stop` | 服务停止 |
| `on_service_restart` | 服务重启 |
| `on_firewall_allow` | 防火墙放行 |
| `on_firewall_del` | 防火墙删除 |
| `on_task_add` | 新增定时任务 |
| `on_task_del` | 删除定时任务 |
| `on_login` | 登录成功 |
| `on_logout` | 注销 |
| `on_shop_install` | 商店安装 |
| `on_disk_low` | 磁盘空间低 |
| `on_cpu_high` | CPU 过高 |
| `on_mem_high` | 内存过高 |
| `on_cron` | 周期任务触发节点 |

## 插件完整示例

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

  - id: disk_usage              # 文件操作 + 命令示例
    desc: Show root disk usage
    script: |
      out = cmd("df -h / | tail -n 1")
      ret(out)

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

## 编写插件的实战建议

- **一个工具一件事**：保持脚本短小、可读，参数用 `params` 声明（前端自动渲染表单）。
- **善用 KV**：把跨调用状态放进 `kv_`，重启不丢。
- **用 `cmd` 但要小心**：`cmd` 直接执行 shell，若用用户输入拼接命令务必转义 / 白名单，避免注入。
- **周期任务控制节奏**：`tasks.every` 单位是秒，合理设置避免过度占用。
- **从示例起步**：从 `plugins/demo.yml` 修改，最快上手。

---

# IotaPanel 兼容运行时

vPanel 额外兼容 **IotaPanel 的独立进程插件协议**：

- 插件是一个独立同级的**进程**，可用任意语言编写（Go、Rust、Python、Node.js、Shell…），崩溃相互隔离。
- 目录结构：`plugins/<name>/manifest.yaml` + `bin/<command>`。
- 面板分配端口并注入 `PLUGIN_PORT` / `PLUGIN_NAME` 等环境变量，网关 `/p/<name>/*` 反向代理到该进程。
- 按需冷启动，空闲自动退出释放内存（`iota.idle_secs` 可配）。
- 支持从 URL / GitHub Release 安装插件包（可选 SHA256 校验），或手动放入目录即装即用。

在线能力：

| 接口 | 说明 |
|------|------|
| `GET /api/iota` | 插件列表 |
| `GET /api/iota/status?name=` | 插件状态 |
| `GET /api/iota/log?name=&n=` | 日志尾部 |
| `POST /api/iota/start` / `stop` / `restart` | 启停重启 |
| `POST /api/iota/keepalive` | 保活开关 |
| `POST /api/iota/uninstall` | 卸载 |
| `POST /api/iota/install_url` | 从 URL 安装（含 sha256） |

### 端口池

默认 `20000` ~ `21999`，可由 `iota.port_lo` / `iota.port_hi` 调整。

---

# 配置参考（panel.yml）

每个字段都有合理默认值。空配置文件也能启动 —— 面板始终保证可用性第一。完整示例见 [panel.yml](./panel.yml)。

以下是全字段说明。

## server：监听与服务端

```yaml
server:
  bind: "0.0.0.0"    # 监听地址
  port: 8080          # 监听端口
  workers: 1          # 固定工作线程数（约束内存）
  backlog: 1024       # 连接队列上限，满则拒绝
  tls:                # 内置 HTTPS
    enabled: false
    cert_file: ""     # 已有证书 PEM；留空自动生成自签
    key_file: ""
    host: "vpanel.local"  # 自签证书的 CN / SAN
  proxies: []         # 路径式反向代理
```

`proxies` 示例：

```yaml
server:
  proxies:
    - prefix: "/app"
      target: "127.0.0.1:8088"
```

## panel：界面

```yaml
panel:
  title: "vPanel"
  subtitle: "极简 · 低内存 HTTP 面板"
  accent: "#2563eb"   # 主题强调色
  theme: "light"      # light | dark
```

## shell：Web 终端

```yaml
shell:
  enabled: true
  cmd: "/bin/sh"      # 或 /bin/bash
  args: []
  columns: 100
  rows: 30
```

## download：软件商店

```yaml
download:
  accel: "https://g.z321.cc.cd/"       # 全局下载加速前缀
  store: "vexify-org/vp-store@main"    # 目录仓库 owner/repo@branch
  docker_dir: "/docker"                # kind:docker 的解压目标
```

## plugins：插件目录

```yaml
plugins:
  dir: "plugins"      # 插件目录，*.yml 自动加载
```

## security：登录安全

```yaml
security:
  enabled: false
  password: ""        # 明文，仅首次设置用；设完存哈希，可留空走向导
  mcp_token: ""       # MCP 独立 Bearer 令牌（可选）
  max_failures: 5
  lock_minutes: 5
  session_hours: 24
  remember_days: 30
  single_session: true
  trust_proxy: false  # 位于受信 HTTPS 反代后置真
```

## database：数据库管理

```yaml
database:
  user: "root"
  password: ""
  bin: "mysql"          # mysql 客户端
  dump: "mysqldump"     # 备份工具
  backup_dir: "<panel_dir>/db-backup"
```

## backup：备份

```yaml
backup:
  dir: "<panel_dir>/backup"
  keep: 5             # 每个来源保留版本数
  cron: "0 3 * * *"   # 定时备份 cron
```

## certs：证书存储

```yaml
certs:
  dir: "<panel_dir>/certs"
  le: false           # 是否启用 acme.sh Let's Encrypt
```

## iota：IotaPanel 兼容运行时

```yaml
iota:
  home: "<panel_dir>/iota"
  prefix: "/p"
  port_lo: 20000
  port_hi: 21999
  idle_secs: 300      # 空闲退出秒数，0 不自动退出
```

---

# API 参考

## 约定

- 基址：`/api/*`。
- 数据查询用 `GET`；系统操作与配置用 `POST`。
- 操作类接口的请求体为表单 `application/x-www-form-urlencoded`。
- 成功返回 `{"ok":true,...}`；失败返回 `{"ok":false,"msg":...}`。

## 系统与监控

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/system` | 系统快照 + 历史曲线 |
| GET | `/api/monitor?n=` | 监控曲线数据（默认 120 点） |
| GET | `/api/info` | 系统信息 |
| GET | `/api/top?n=` | 资源 TOP |
| GET | `/api/snapshot` | （随 `/api/system`） |

## 进程与连接

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/processes` | 进程列表 |
| POST | `/api/process/kill` | 结束进程（`pid`） |
| GET | `/api/conns` | 网络连接 |
| POST | `/api/conn/kill` | 按端口强杀连接（`port`） |

## 服务与定时任务

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/services` | 服务列表 |
| POST | `/api/service/action` | `name`, `action=start\|stop\|restart` |
| GET | `/api/tasks` | 定时任务列表 |
| POST | `/api/tasks/add` | `schedule`(5 段 cron), `command` |

## 防火墙

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/firewall` | 规则列表 |
| POST | `/api/firewall/add` | `action=allow\|deny`, `port`, `proto`, `ip` |
| POST | `/api/firewall/del` | `id` 或 `port` |
| POST | `/api/firewall/enable` | 开启 ufw |
| POST | `/api/firewall/disable` | 关闭 ufw |

## 文件与日志

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/files?path=` | 目录列表 |
| GET | `/api/file/read?path=` | 读取文件 |
| POST | `/api/file/delete` | `path` |
| POST | `/api/file/save` | `path`, `data` |
| POST | `/api/file/upload?path=` | body 为原始字节，上限 8MB |
| GET | `/api/log/tail?file=&n=` | 日志尾部 |
| GET | `/api/log/follow?file=&pos=` | 日志增量 |
| GET | `/api/disk/top?path=&n=` | 磁盘占用 TOP |

## 软件商店与插件

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/shop` | 商店目录 |
| POST | `/api/shop/install` | 安装（`id`） |
| GET | `/api/plugins` | 插件列表 |
| GET | `/api/plugin/kv` | 插件 KV 一览 |
| GET | `/api/plugin/store` | 插件市场目录 |
| POST | `/api/plugin/store/install` | 安装市场插件（`id`） |
| POST | `/api/plugin/<p>/<t>` | 调用插件工具（支持 `enable` / `disable` / `uninstall`） |

## 数据库

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/db/status` | 连接状态 |
| GET | `/api/db/databases` | 数据库列表 |
| GET | `/api/db/users` | 用户列表 |
| GET | `/api/db/backups` | 备份列表 |
| POST | `/api/db/create_db` | `name`, `charset` |
| POST | `/api/db/drop_db` | `name` |
| POST | `/api/db/create_user` | `user`, `pass`, `host` |
| POST | `/api/db/drop_user` | `user`, `host` |
| POST | `/api/db/grant` | `db`, `user`, `host` |
| POST | `/api/db/backup` | `db` |
| POST | `/api/db/restore` | `db`, `file` |
| POST | `/api/db/reset_root` | `password` |

## 证书与环境

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/ssl` | 证书列表 |
| POST | `/api/ssl/import` | 导入（`name`, `fullchain`, `privkey`） |
| POST | `/api/ssl/self_signed` | 自签（`name`, `domain`, `days`） |
| POST | `/api/ssl/le_issue` | Let's Encrypt（`name`, `domain`, `webroot`） |
| POST | `/api/ssl/apply` | 应用到站点（`site`, `cert`, `upgrade`） |
| GET | `/api/env` | 环境状态 |
| POST | `/api/env/install` | 安装运行时 |
| POST | `/api/env/service` | 管理运行时服务 |

## 备份与安全

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/backup` | 备份列表 |
| GET | `/api/security/bans` | 封禁列表 |
| GET | `/api/security/hardening` | 加固状态 |
| POST | `/api/security/ban` | 封禁（`ip`） |
| POST | `/api/security/unban` | 解封（`ip`） |
| POST | `/api/security/brute` | 防爆破扫描 |
| POST | `/api/security/harden` | SSH 加固 |
| POST | `/api/security/unharden` | SSH 回滚 |
| GET / POST | `/api/security/waf` | WAF 状态 / 启用 |
| POST | `/api/security/waf/disable` | 关闭 WAF |

## 站点与 Nginx

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/website` | 站点列表 |
| POST | `/api/website/create` | `name`, `domain`, `listen`, `php`, `php_version` |
| POST | `/api/website/toggle` | `name`, `enable` |
| POST | `/api/website/delete` | `name`, `drop_root` |
| POST | `/api/website/rewrite` | `name`, `kind` |
| GET | `/api/nginx` | Nginx 配置列表 |
| POST | `/api/nginx/add` | `name`, `server_name`, `listen`, `target` |
| POST | `/api/nginx/toggle` | `name`, `enable` |
| POST | `/api/nginx/delete` | `name` |
| POST | `/api/nginx/reload` | 重载 Nginx |
| GET / POST | `/api/autostart` | 自启状态 / 开关（`name`, `enable`） |

## 反向代理与告警

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/proxy` | 反代列表 |
| POST | `/api/proxy/add` | `prefix`, `target` |
| POST | `/api/proxy/del` | `prefix` |
| GET | `/api/alert` | 告警配置 + 实时资源 |
| POST | `/api/alert/save` | 保存告警配置 |
| POST | `/api/alert/enable` / `disable` | 开关告警 |
| POST | `/api/alert/test` | 测试邮件 |

## Iota 运行时

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/iota` | 插件列表 |
| GET | `/api/iota/status?name=` | 插件状态 |
| GET | `/api/iota/log?name=&n=` | 日志尾部 |
| POST | `/api/iota/start` / `stop` / `restart` | `name` |
| POST | `/api/iota/keepalive` | `name`, `on` |
| POST | `/api/iota/uninstall` | `name` |
| POST | `/api/iota/install_url` | `url`, `sha256` |

## MCP 与健康检查

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/mcp` | MCP 端点（`initialize` / `tools/list` / `tools/call`） |
| GET | `/p/<name>/*` | Iota 插件网关反代 |
| GET | `/health` | 健康检查 → `ok` |
| GET | `/metrics` | 请求 / 并发 / 内存统计 |

> 进程 / 服务 / 防火墙 / 定时任务等操作需要 `root` 权限，并依赖 `systemd` / `ufw` / `crontab` 可用。

---

# 内存设计

- 手工编写的 HTTP 服务器：每个连接服务完立即关闭，不残留缓冲区。
- 固定线程池 + 有界队列：高并发下连接在内核层背压或丢弃，内存不随并发增长。
- 系统命令（`systemctl` / `ufw` / `crontab` / `df`）作为一次性子进程运行，请求结束即释放。
- 监控曲线用定长环形缓冲区；系统快照与进程列表按需读取、立即释放。
- 多线程堆分配器 arena 压缩到 1 个（`MALLOC_ARENA_MAX=1`），进一步压低常驻内存。
- 插件 DSL 每次执行新建解释器、跑完即释放。

---

# 性能与压测

基准：空闲约 **0.8MB**；300 并发请求（开启应用商店与 MCP）后约 0.9MB —— 仍远在 10MB 预算之内。

| 场景 | 常驻内存（约） |
|------|----------------|
| 空闲 | 0.8MB |
| 300 并发（商店 + MCP 开启） | 0.9MB |
| 长期运行 | 稳定在预算内 |

> 实际占用会随访问的插件 / 功能而定，但架构将其严格收窄在有界资源内。

---

# 安全最佳实践

- **公网务必 HTTPS**：默认明文 HTTP 监听 `:8080`，不经加密代理直接暴露公网时，管理员口令与会话可能被网络嗅探。用 Nginx / Caddy 反代终结 TLS，或开启[内置 TLS](#内置-https)。
- **开启登录保护**：设 `security.enabled: true`，走初始设置向导设置强密码。
- **MCP 令牌**：需要外部 AI 接入时，配 `security.mcp_token` 作为独立 Bearer 令牌。
- **保持最小暴露**：`server.bind` 视情况改为 `127.0.0.1`，仅在需要时对外。
- **插件审慎**：DSL 插件可执行任意 `cmd`，只安装可信来源的插件。
- **WAF / 防爆破**：开启 SSH 加固、防爆破扫描，降低被爆破风险。
- **定期备份**：配置好目录 + 数据库的定时备份。

---

# 常见问题（FAQ）

**Q：为什么不用数据库 / Redis / 运行时？**
A：极简第一。所有状态（登录、KV、告警、配置）都以文本文件持久化，重启即恢复，无需任何附加服务。

**Q：提示 `./vpanel: GLIBC_2.39 not found`？**
A：使用 release 页的 **静态 musl** 二进制（`vpanel-linux-*`）即可，它们不依赖目标机 GLIBC 版本。

**Q：加密 SMTP（STARTTLS/SSL）显示"当前构建未启用"？**
A：需要用 `--features tls` 构建（参考[从源码构建](#从源码构建)）。release 二进制已默认启用。

**Q：哪些功能需要 root？**
A：进程 / 服务 / 防火墙 / 定时任务等系统级操作需要 root，并依赖 `systemd` / `ufw` / `crontab`。

**Q：如何让 AI 管理我的服务器？**
A：把 MCP 客户端指向 `http://<host>:8080/mcp`，详见[AI 工具（MCP）](#ai-工具mcp)。

**Q：页面按钮点了提示"缺少 name/action/ID"？**
A：请升级到 1.6.x（已修复表单 `Content-Type` 与参数解析）。操作类接口请用 `application/x-www-form-urlencoded` 编码。

**Q：能跑在我 512MB 的 VPS 上吗？**
A：能。常驻约 0.4MB、峰值约 3MB，对低配机器几乎无感知。

**Q：数据放在哪？**
A：默认当前工作目录（pid / 日志 / 配置 / 证书 / 备份 / kv 均在其下），可用 `VPVPANEL_DIR` 改变。

**Q：支持哪些架构？**
A：x86_64（amd64）、ARM64（aarch64）、ARMv7（armhf），均静态编译。

**Q：内置 TLS 和反代的区别？**
A：内置 TLS 是面板自身的 HTTPS 终结；路径式反代是把面板端口上的路径前缀代理到任意本机服务。两者可叠加。

---

# 排障指南

### 打不开页面

1. 确认进程在跑：`vpanel status`。
2. 确认端口占用 / 防火墙放行。
3. 看日志：`vpanel log`。

### 操作报"缺少 name/action/ID"

- 升级到 1.6.x。
- 确认操作接口使用 `application/x-www-form-urlencoded` 编码。

### 提示 GLIBC 版本错误

- 换用 release 静态二进制。

### SMTP 测试失败

- 确认 `--features tls` 已启用（加密模式前提）。
- 检查主机 / 端口 / 认证 / 授权码（很多邮箱需"授权码"而非登录密码）。

### 交叉编译 ring 报错

- 确认对应架构的 `gcc` 已安装。
- 遇到缓存冲突先 `cargo clean`。

---

# 贡献指南

- 所有功能均以源码为准，欢迎提交 Issue / PR。
- 代码保持极简与低内存原则：不改代码编译产物大小 / 常驻内存的结构性回归。
- 插件生态欢迎共享到商店仓库。

---

# 路线图

- 完善内置 TLS/ACME 自动化签发。
- 插件市场更丰富的官方包与一键模板。
- 更多运行时（Node/Python 等）的一键环境管理。
- 图形化的备份恢复 / 站群管理增强。
- 更多安全基线（fail2ban 集成、自定义 WAF 规则）。

---

# 许可

[Apache-2.0](./LICENSE)

---

**Powered By Vexify.**