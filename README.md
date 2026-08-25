# vPanel · One Server, One Entry, Total Control

> A lightweight server console forged from scratch in Rust — **~1MB** idle, peaks under **2MB** under load, hard-budgeted at **10MB**.
> Minimal by design, boundless by extension: processes, services, security, tasks, app store, AI — all in one screen.

**vPanel** is a single-binary, YAML-driven, zero-dependency panel. No database, no cache, no heavy runtime. Drop it, run it, own it.

---

## Why vPanel

- **Light, to the bone** — A statically-linked binary, ~1MB idle. Your low-end VPS won't even feel it.
- **Complete, out of the box** — Monitoring / processes / services / firewall / cron / app store / file manager / live logs / web terminal. Everything you need, nothing you don't.
- **Alive, with AI** — Built-in `/mcp` endpoint (MCP Streamable HTTP). Any AI client can drive the panel directly. This isn't just a dashboard — it's a server with a brain.
- **Extensible, through plugins** — A homegrown minimalist DSL with a micro-scripting language. One YAML file, one capability. 20 event hooks dancing to the heartbeat of your server.

---

## Quick Start

### Pre-built binaries (zero dependencies, recommended)

Download from [Releases](https://github.com/vexify-org/vPanel/releases) for your architecture: `x86` / `x64` / `arm` / `arm64`.

```bash
chmod +x vpanel-<arch>
./vpanel-<arch>          # auto-discovers panel.yml / config.yml in CWD
```

Verify integrity:

```bash
sha256sum -c SHA256SUMS
```

### Alpine Linux (apk)

Statically linked against musl, ready for Alpine out of the box. Signed APK packages are included in the release:

```bash
<<<<<<< HEAD
# 将公钥放入 /etc/apk/keys 后，直接从本地包安装
apk add vpanel-x86_64-1.5.0-r0.apk   # 按架构选择
=======
# Copy the public key to /etc/apk/keys, then install locally
apk add vpanel-aarch64-1.5.0-r0.apk   # pick your arch
>>>>>>> da48823 (feat: 优化HTTP服务YML配置内存)
```

### Build from source

```bash
cargo build --release
./target/release/vpanel
```

Open `http://<host>:8080/` in your browser.

---

## Features at a Glance

| Module | Description |
|--------|-------------|
| System Monitor | Real-time CPU / memory / disk / network graphs + load (bounded ring buffer) |
| System Info | OS / kernel / arch / CPU model & cores / temperature / partition details (read-only) |
| Network Connections | Connection states & port aggregation, with kill-by-port capability |
| Live Logs | Browser-based tail -f for any file, incremental pull, bounded & efficient |
| File Manager | Directory listing / text editing / upload / download / delete |
| Process Manager | `/proc`-based, sorted by RSS, with kill capability |
| Service Manager | systemctl-based start / stop / restart |
| Firewall | ufw-based port allow / deny |
| Cron | crontab-based add / list / delete |
| App Store | One-click install of common software; remote catalog with local fallback |
| Plugins | Minimalist DSL + homegrown scripting language, injected into both UI and MCP |
| AI Tools | `/mcp` endpoint, ready for any AI client (including all plugin tools) |
| Web Terminal | WebSocket + PTY, native shell in your browser |
| Health | `/health` → `ok`, `/metrics` → request/concurrency/memory stats |

> Process / service / firewall / cron operations require `root` privileges and `systemd` / `ufw` availability.

---

## AI Tools (MCP)

The panel exposes a **MCP Streamable HTTP** endpoint at `POST /mcp`.

- Supports `initialize`, `tools/list`, `tools/call`.
- Point your Claude / Cursor / any MCP client to `http://<host>:8080/mcp` and the AI gains instant access to: system monitoring, process management (including kill), service management (start/stop/restart), firewall rules, cron tasks, and every plugin tool (named `p_<plugin>_<tool>`).
- The built-in "AI Tools" page provides connection info, tool self-check, and an interactive test console.

**808 built-in MCP tools** (pure functions, each independent), plus the plugin system — **1,244 tools total**. An entire operations army, commanded by AI.

---

## Plugins (Minimalist DSL + Micro Scripting Language)

Plugins are YAML files dropped into the plugin directory (`plugins/` by default) — auto-loaded on detection. The scripting language is a homegrown micro-language: **indent-based blocks + control flow**, supporting conditions / loops / comparisons, tool arguments, KV persistence, and a text/math function library. No heavy runtime; each execution spawns a fresh interpreter that dies when done.

**Language features**

- Variables (string / number / bool), assignment, arithmetic `+ - * / %`, string `+` concat
- Comparisons `== != < <= > >=` and logic `and or not`
- Control flow: `if / else`, `for i in range(n)`, `while`, `break / continue`; blocks delimited by indentation, terminated by `end`
- Tool arguments: `arg("id")` / `has_arg("id")`
- KV persistence: `kv_set("k","v")` / `kv_get("k")`, namespaced per plugin, auto-persisted, survives restarts
- Built-in functions: `cmd` / `fetch` / `ret` / `log` / `env` / `var`, text `len/substr/split/atoi/itoa/upper/lower/trim`, math `min/max/round/ceil/floor`, structured `json("...")`

**Management capabilities**

- Enable / disable: `POST /api/plugin/<name>/enable` / `disable`, state persisted
- Lifecycle: `POST /api/plugin/<name>/uninstall` removes the manifest and hot-reloads
- Online install / update: `GET /api/plugin/store` lists catalog; `POST /api/plugin/store/install` downloads and hot-reloads
- Custom forms / pages: declare `params` on a tool, and the frontend auto-renders a form while MCP auto-generates `inputSchema`
- Tools injected into both UI and MCP, periodic tasks, 20 event hooks

Example plugin `plugins/demo.yml`:

```yaml
name: demo
version: 1.1.0
tools:
  - id: greet                 # /api/plugin/demo/greet and MCP p_demo_greet
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
  - id: counter                 # KV persistence demo
    desc: KV counter (survives restarts)
    script: |
      c = atoi(kv_get("count")) + 1
      kv_set("count", itoa(c))
      ret("Ran " + itoa(c) + " times")
tasks:                          # Built-in periodic tasks (no crontab needed)
  - id: heartbeat
    every: 10
    script: |
      log("Heartbeat " + cmd("date \"+%H:%M:%S\""))
hooks:                          # One of 20 event hooks
  - event: on_init
    script: |
      log("Plugin loaded")
```

---

## Configuration

Every field has a sensible default. An empty config file works — the panel will still start. See [panel.yml](./panel.yml) for a full example.

```yaml
server:
  bind: "0.0.0.0"   # Listen address
  port: 8080         # Listen port
  workers: 4         # Fixed worker threads (keeps memory bounded)
  backlog: 1024      # Connection queue limit; rejects when full

panel:
  title: "vPanel"
  subtitle: "Minimal · Low-Memory HTTP Panel"
  accent: "#2563eb"  # Theme accent color
  theme: "light"     # light | dark

shell:               # Web terminal
  enabled: true
  cmd: "/bin/sh"     # or /bin/bash
  args: []
  columns: 100
  rows: 30

download:              # App store
  accel: "https://g.z321.cc.cd/"     # Global download acceleration prefix
  store: "vexify-org/vp-store@main"  # Catalog repo (owner/repo@branch)

plugins:
  dir: "plugins"        # Plugin directory, *.yml auto-loaded
```

---

## Memory Design

- Hand-written HTTP server: each connection is closed immediately after serving; no lingering buffers.
- Fixed thread pool + bounded queue: under high concurrency, connections are back-pressured at the kernel level or dropped. Memory does not grow with concurrency.
- System commands (`systemctl` / `ufw` / `crontab` / `df`) run as one-shot subprocesses, released as soon as the request completes.
- Monitoring graphs use fixed-length ring buffers. System snapshots and process lists are read on-demand and released immediately.

Benchmarked: ~0.8MB idle; ~0.9MB after 300 concurrent requests (with app store & MCP enabled — still well under the 10MB budget).

---

## API Reference (/api/*)

- GET `/api/system` — System snapshot + history curves
- GET `/api/processes` — Process list
- GET `/api/services` — Service list
- GET `/api/firewall` — Firewall rules
- GET `/api/tasks` — Cron tasks
- GET `/api/shop` — App store catalog
- POST `/api/process/kill` — `pid`
- POST `/api/service/action` — `name`, `action=start|stop|restart`
- POST `/api/firewall/add` / `/api/firewall/del` — `port`
- POST `/api/tasks/add` — `schedule`(5-field cron), `command`
- POST `/api/shop/install` — `id`

Also: `POST /mcp` — MCP endpoint (`initialize` / `tools/list` / `tools/call`).

---

## License

[Apache-2.0](./LICENSE)

---

**Powered By Vexify.**
