# 更新记录（CHANGELOG）

> 面板：**vPanel** —— 纯 Rust 编写的极简低内存 HTTP 管理面板。
> 记录约定：按 `语义化版本` 组织，`v1.3.0` 为当前主干版本；往下为规划中的目标版本（对标宝塔能力补全）。
> 内存目标：常驻内存 ≤ 10MB。

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