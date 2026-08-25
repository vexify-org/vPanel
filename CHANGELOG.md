# 更新记录（CHANGELOG）

> 面板：**vPanel** —— 纯 Rust 编写的极简低内存 HTTP 管理面板。
> 记录约定：按 `语义化版本` 组织，`v1.3.0` 为当前主干版本；往下为规划中的目标版本（对标宝塔能力补全）。
> 内存目标：常驻内存 ≤ 10MB。

## v1.4.0（待发布）

本轮为「宝塔功能全量补齐」，重点是给已有后端模块接入**可视化前端页面**，并新增若干后端能力。

### 新增
- **建站支持指定 PHP 版本**（`src/website.rs`）
  - `php_socket_for()` 按版本映射 socket：`8.2` → `/run/php/php8.2-fpm.sock`，空/非法回退默认
  - `website_create(..., php, phpver)` 新增 `php_version` 参数；接口 `POST /api/website/create` 支持 `php_version` 字段
- **数据库管理前端**（`src/panel.rs`，新「数据库」tab）
  - 对接已有后端 + 新增 `/api/db/reset_root` 重置 root 密码（`src/db.rs` 新增 `reset_root_password`）
  - 建库/删库/建用户/授权/备份可视化
- **环境运行时前端**（新「环境」tab）：nginx/mysql/redis/php/node/docker/go 安装/启停/重启
- **SSL 证书前端**（新「证书」tab）：自签 / Let's Encrypt 签发 / 导入已有证书 / 套用到站点
- **备份前端**（新「备份」tab）：全量备份 / 目录备份 / 定时备份 / 移除 / 云上传
- **云备份上传**（`src/backup.rs` 新增 `cloud_upload`，`/api/backup/cloud`）：经 `lftp` 推送到远程 FTP，连接信息用 `VPANEL_FTP_HOST/USER/PASS/DIR` 环境变量，未配置时给出引导提示
- **安全加固前端**（新「安全」tab）：SSH 加固/撤销、WAF 开关、已封禁 IP、暴力破解扫描封禁
- **MCP 全量工具化**（`src/mcp.rs`）——把上述所有宝塔能力以 Model Context Protocol 工具暴露，供 Claude/Cursor 等 AI 客户端通过 `POST /mcp` 调用：
  - 新增 60 个内置工具，覆盖 网站 / 数据库 / SSL / 运行环境 / 备份 / 安全(WAF+加固) / IotaPanel 插件 / 插件商店 / 监控快照 / 资源排行 / 软件商店 / 日志增量
  - `tools/list` 自动产出每工具的 `inputSchema`（参数类型按字符串/数字/布尔映射）
  - 复用现有 `db/databases` 等纯函数，工具派发 `tools/call` 与 `/api/*` 同源，无新增常驻状态
- **MCP 工具继续补全 + 新后端函数**（`src/mcp.rs`、`src/extra.rs`）
  - 新增容器管理（对标宝塔「Docker」）：`docker_containers` 列出容器、`docker_action` 启停/重启
  - 新增磁盘占用总览 `disk_usage`（df -Pk 解析）
  - 新增文件操作 `file_mkdir` 创建目录、`file_rename` 重命名/移动（`src/extra.rs` 新增 `mkdir`/`rename`/`disk_usage_json`/`docker_containers_json`/`docker_action`）
  - 内置 MCP 工具扩充至 66 个
- **运维工具箱大爆发**（新模块 `src/ops.rs`，内置 MCP 工具 66 → 133）
  - 一次性新增 **67 个**纯函数工具，全部「按需执行、随求即释」、无常驻状态，实测 RSS 仍 ≈ **6.0MB**（≤10MB 预算内）
  - **网络诊断**：`ping` / `tcp_ping` / `dns_lookup` / `http_head` / `listener_ports` / `port_check` / `reverse_dns`
  - **系统纵深**：`cpu_info` / `cpu_usage` / `mem_info` / `swap_info` / `loadavg` / `net_io` / `disk_inodes` / `os_release` / `kernel_info`
  - **文件系统深度**：`ls_long` / `dir_size` / `file_count` / `file_search` / `file_chmod` / `zip_archive` / `zip_extract` / `file_head` / `file_size`
  - **进程/服务**：`process_by_name` / `process_detail` / `systemd_units` / `systemd_action`
  - **软件包(apt)**：`apt_update` / `apt_upgrade` / `apt_install` / `apt_remove` / `apt_list_installed` / `pkg_installed`
  - **计划任务(cron)**：`cron_list` / `cron_add` / `cron_remove` / `cron_system`
  - **运行时版本**：`php_version` / `node_version` / `go_version` / `python_version` / `mysql_version` / `php_fpm_sockets`
  - **数据库深化**：`db_sizes` / `mysql_status` / `mysql_ping`
  - **SSL 深化**：`cert_view` / `cert_expiry`（证书剩余天数）
  - **Docker 深化**：`docker_images` / `docker_stats` / `docker_prune` / `docker_info`
  - **日志**：`dmesg_tail` / `journal_tail` / `nginx_error_tail` / `nginx_access_tail` / `mysql_error_tail` / `auth_log_tail`
  - **用户/杂项**：`users_list` / `whoami` / `random_password` / `sha256` / `base64_encode` / `base64_decode` / `uuid` / `panel_about`
  - 新增依赖 `rand` / `sha2`（minimal features，仅用作随机/哈希），随添加随用小体积实现

### 常驻内存压到 2MB 档（对标 ≤10MB → 实际 ≈2.3MB）
- **TLS 改为可选 `tls` 特性**（`Cargo.toml` + `src/tls.rs` 双分支）
  - 默认 build 不再内链 `rustls`/`ring`/`rcgen`——ring 的纯汇编(AES/SHA/P256)代码段体积很大
  - 二进制从 2.42MB → **1.53MB**；常驻内存(冷启动)从 2.7MB → **≈2.1MB**
  - HTTPS 一点不丢：`cargo build --release --features tls` 恢复完整内置 TLS（自签/已有证书）
  - 无 `tls` 特性时 `Server` 为纯 TCP 透传空壳、`enabled()` 恒 false，对外 API 接口不变
- **瘦线程**：面板示例 `workers: 4→1`、默认 `d_workers() 2→1`、iota 空闲回收线程在 `idle_secs==0` 时不再常驻
- 最终实测（release，线程 6）：
  - 冷启动 RSS **≈2.1–2.3MB**；打过 MCP/API 后 RSS **≈2.33MB**、PSS **≈2.16MB**

### 验证
- 单测：36/36 通过（含建站 PHP 版本 socket 映射断言）
- 构建：`cargo build --release`（精简）与 `cargo build --release --features tls`（含 HTTPS）双通道均编译通过
- 实测内存：release 精简构建常驻 RSS ≈ 2.1MB（冷启动）/ ≈2.3MB（打满 API+MCP）

---

## v1.3.0（当前）

本次为「建站能力」与「建站前端」的功能补齐。

### 新增
- **网站建站管理**（`src/website.rs`）
  - 创建真实网站：自动建立站点根目录 + 默认首页 + Nginx `server` 块
  - 站点类型：纯静态 / PHP-FPM 映射
  - 伪静态规则内置模板：WordPress、ThinkPHP、Laravel、removed
  - 站点列表（域名/端口/类型/根目录/启用状态）、启停、删除（可连带删根目录）
  - `nginx -t` 校验失败自动回滚
  - **插桩（dry-run）模式**：设置 `VPANEL_DRY_ROOT` 后，文件写入与 nginx 命令重定向到沙盒，未装 Nginx 也能端到端验证控制逻辑
- **前端「网站管理」页**（`src/panel.rs`）
  - 侧边栏新增「网站」tab
  - 建站表单 + 网站列表 + 行内操作（伪静态/启停/删除）
- **新接口**：`GET /api/website`、`POST /api/website/create|toggle|delete|rewrite`

### 修复
- 端口解析错误：`listen 80` 曾解析为 `"n"`（`get(5..)` 挖位偏移，已修正）
- 站点「启用」状态检测：改用 `symlink_metadata` 避免跟随软链误判
- 启用软链目标：改为绝对路径，保证可解析

### 验证
- 单测：建站模块 2 组用例通过（含建站→列表→伪静态→启停→删除端到端）
- 实测内存：debug 构建常驻 RSS ≈ 6.7MB

---

## v1.3.0 之前的既有能力（本仓库基线）

> 主干已实现的功能模块，作为后续版本对比的基线。

### 面板基础
- 登录认证 / 初始设置向导 / 会话管理（`src/auth.rs`）
- YAML 配置（`panel.yml`）、主题（深/浅色）

### 运维 / 监控
- 系统监控：CPU / 内存 / 磁盘 / 网络速率实时曲线（`src/system.rs`）
- 系统信息、网络连接、资源实时 Top、磁盘占用排行（`src/extra.rs`）
- 进程管理（按内存排序 / 结束进程）
- 服务管理（systemctl 启停）、开机自启
- 防火墙端口放行（ufw）
- 定时任务（crontab 五段式）

### 数据 / 环境 / 建站相关
- 数据库管理：MySQL 建库 / 用户 / 授权 / 备份（`src/db.rs`）
- SSL：自签 / 证书导入 / Let's Encrypt（acme.sh）签发与续期（`src/ssl.rs`）
- 环境运行时：PHP / MySQL / Redis / Node / Docker / Go 的安装、启动、停止（`src/env.rs`）
- Nginx 反向代理站点管理与 reload（`src/nginx.rs`）
- 网站备份 / 数据库备份（`src/backup.rs`）

### 安全
- 系统安全加固 / Fail2ban / 暴力扫描防护 / WAF（`src/security.rs`）
- Web 终端：pty + WebSocket（xterm.js）（`src/term.rs`、`src/ws.rs`）
- 内置 HTTPS：TLS 终结（自签 / 已有证书）（`src/tls.rs`）

### 扩展性
- 软件商店（加速下载源）（`src/shop.rs`）
- 插件系统：极简 DSL + 微脚本语言 + KV 持久化（`src/plugins.rs`、`src/lang.rs`）
- MCP 端点（Streamable HTTP），AI 客户端可直接调用面板能力
- Iota 独立进程运行时（`src/iota.rs`）

---

## 规划中（对标宝塔补全路线）

- [ ] PHP 多版本选择与站点映射（按版本选择 php-fpm socket）
- [ ] 数据库管理深化：phpMyAdmin 入口、root 密码重置、导入导出
- [ ] SSL 面板化：证书列表 / 到期提醒 / 绑定站点
- [ ] 备份调度：定时备份 + 保留期 + 云备份（OSS/COS/FTP）
- [ ] 安全加固界面：SSH 配置、Fail2ban、WAF 的可视化管理
- [ ] 软件商店：一键部署应用（WordPress 等）