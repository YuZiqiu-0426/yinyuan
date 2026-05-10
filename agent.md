# agent.md

This file is the **single repository guide** for human contributors and AI coding agents (Claude Code, Cursor, etc.): **engineering conventions**, **process**, **layout**, **build/test commands**, and **architecture** for YinYuan / `y2m-rs`.

Formerly split across `CONVENTIONS.md` and `CLAUDE.md`; those files now redirect here.

---

## Document roles and conflict priority

1. **`docs/当前实现说明.md`** — authoritative **what the code does today**.
2. **`docs/工作进度.md`** — priority and task status.
3. **`agent.md`** (this file) — how to work in the repo, commands, architecture overview, engineering rules.
4. Other requirements / design / API / runbook documents under `docs/`.

If this file disagrees with `docs/当前实现说明.md`, **trust the implementation doc** and update `agent.md`.

---

## Language

- Human-facing prose may be **Chinese (Simplified)** where existing docs already use it.
- **Code identifiers, commit messages, and public API names** stay in English unless an established exception exists.

---

## Code size

1. **Functions**: at most **50 lines** each (including blanks and comments). Split into helpers when needed.
2. **Files**: at most **500 lines** per source file. Split by responsibility when approaching the limit.

---

## Git workflow

3. **After completing a task**: land the outcome, then **`git add`** relevant paths and **`git commit`**. Use a clear, full-sentence message describing *what* changed and *why*. Automation should commit when finishing a coherent task, not only print commands for humans.

---

## Runtime and tooling

4. **tmux**: Prefer **tmux** sessions/windows for long-lived local servers and watchers; avoid scattering them across unrelated tabs without a layout.
5. **Frontend package manager**: Under **`frontend-monorepo/`**, use **pnpm** only (`pnpm install`, `pnpm run <script>`). Do not introduce npm or Yarn for day-to-day work unless an exception is documented here.

---

## Project conventions

6. **Repository layout**: Root = docs and coordination. Rust workspace = **`y2m-rs/`**. Frontend apps = **`frontend-monorepo/`**.
7. **Rust commands**: Run **`cargo`** from **`y2m-rs/`** (or `--manifest-path y2m-rs/Cargo.toml`). The repo root is not a Rust workspace root.
8. **Implementation baseline**: Use **`docs/当前实现说明.md`** when describing current behavior.
9. **What to build next**: Check **`docs/工作进度.md`** first.
10. **Admin UI**: **`frontend-monorepo/apps/y2-manage`** (Angular). User chat Web client removed; user interaction is **CLI-only** via **`y2m`**. Multi-Agent orchestration shares the same WebSocket transport; application messages may use **`agent-collab`** on top of **`EventType::Json`** (see `docs/agent-collab-protocol-v1.md`).
11. **Messaging baseline**: Preserve behaviors documented in **`docs/当前实现说明.md`** for routing, reconnect, file transfer, and tests unless you intentionally change the product and update that doc.

---

## Process conventions

12. **Read-before-change**: For non-trivial work, read **`docs/当前实现说明.md`**, **`docs/工作进度.md`**, and the relevant sections of **this file** (layout, commands, tests).
13. **Change against reality**: If design or navigation text disagrees with **`docs/当前实现说明.md`**, align implementation and docs to the implementation doc first.
14. **Document update order** when behavior changes:
    1. `docs/当前实现说明.md`
    2. `docs/工作进度.md`
    3. **`agent.md`** if commands, layout, tests, or engineering rules changed
    4. Related requirement / design / API / runbook docs
15. **Testing**: After a coherent code change, run the narrowest meaningful check first, then broader workspace tests when risk warrants it.
16. **Long-lived processes**: Keep servers/watchers in **tmux** where practical.

---

## Where this file is referenced

- **Cursor**: `.cursor/rules/yinyuan-conventions.mdc` points here for assisted sessions.
- **Stubs**: `CONVENTIONS.md` and `CLAUDE.md` redirect to this file for backward compatibility.
- Security and auth rules live under **`docs/`** (e.g. `docs/加密验证方案.md`, `docs/权限矩阵与默认角色模板-v1.md`). If they conflict with older requirement docs such as `需求v1.md`, **prefer the security docs**.

---

## Repository layout (docs index)

The Rust workspace is **`y2m-rs/`**. The repo root holds coordination and design docs, for example:

- `需求v1.md` — original requirements (v1).
- `docs/当前实现说明.md` — implementation status; read before new work.
- `docs/加密验证方案.md` — transport encryption, device trust, IP allowlists, key lifecycle.
- `docs/统一认证中心详细设计-v1.md` — auth-service architecture, JWT, CLI device-factor auth.
- `docs/统一认证中心API定义-v1.md` — auth API contracts and error codes.
- `docs/权限矩阵与默认角色模板-v1.md` — RBAC templates and atomic permissions.
- `docs/配置与密钥管理规范-v1.md` — env secrets, JWT rotation, baselines.
- `编译部署文档.md` — build and deployment notes.
- `使用手册.md` — user manual.
- `y2m-rs/docs/quickstart.md` — quickstart for server and CLI.

---

## Build and run

```bash
cd y2m-rs
cargo build                      # debug workspace
cargo build --release            # release in target/release/
cargo build -p y2m               # CLI only
cargo build -p y2m-server       # server only
```

Binaries:

- **`y2m-server`** — WebSocket relay. Default `127.0.0.1:8080`; override with **`Y2M_SERVER_ADDR`**. No CLI flags.
- **`y2m`** — CLI: **`init | run | send | chat`**.

Typical loop: start server, `y2m init --config alice.json ...`, then `y2m chat --config alice.json` (or `y2m run` for passive mode). Full walkthrough: **`y2m-rs/docs/quickstart.md`**.

---

## Tests

```bash
cd y2m-rs
cargo check --workspace
cargo test --workspace
cargo test -p y2m
cargo test --test file_transfer_v3
cargo test --test cli_process_e2e -- --nocapture
```

**Integration tests** (`y2m-rs/tests/`):

1. **In-process** (`text_e2e.rs`, `json_e2e.rs`, `file_transfer_v3.rs`, …): `y2m_server::serve_with_listener_and_config` on ephemeral ports, `y2m_client_core` APIs. Helpers in `tests/support/mod.rs`.
2. **CLI e2e** (`cli_process_e2e.rs`, `cli_file_*_e2e.rs`, `cli_reconnect_e2e.rs`, …): real binaries via `tests/support/cli.rs`. Slower; some use **`serial_test`**.

Prefer in-process tests unless you need CLI parsing, stdin-driven chat, or process-level reconnect.

---

## Architecture

```
y2m-rs/
  crates/common/       -> y2m-common       (protocol types)
  crates/server/       -> y2m-server       (WebSocket relay)
  crates/client-core/  -> y2m-client-core  (transport, framing, plugins)
  src/                 -> y2m              (CLI)
```

### Protocol (`crates/common`)

Protocol **`v3`** (`PROTOCOL_VERSION`). Control plane: JSON **`Packet<T>`** with `kind` of `init | init_ack | heartbeat | heartbeat_ack | event | ack | error`. **`EventType`**: `text`, `json`, `command`, `command_result`, `file_offer`, `file_accept`, `file_reject`, `file_complete`, `file_abort`. File bytes: **`BinaryChunkHeader`**, magic **`Y2MB`**, `frame_type` 1 = file chunk on the same WebSocket. **`Endpoint { groupName, clientName }`**. **`Endpoint::server()`** reserved for server-originated packets.

### Server (`crates/server`)

**`ws.rs`**: axum WebSocket; **`init::handle_init`** validates init, rejects duplicate `(group, client)`, returns **`init_ack`**. **`router.rs`**: `text`, `json`, `command`, `command_result`, and all **`file_*`** support **unicast** (`target.clientName` set) or **group broadcast** (`target.clientName` omitted; same group, all sessions except sender — see **`docs/当前实现说明.md`** §3.2). **`session.rs`**: **`SessionStore`**. **`transfer.rs`**: **`TransferRegistry`**; **binary chunks** only after each receiver leg sends **`file_accept`**. Heartbeat timeout closes the connection and frees the name.

### Client core (`crates/client-core`)

**`ClientCore::connect()`** → WS, **`init`**, **`init_ack`**, **`ClientRuntime`**. **`runtime.dispatch_next()`** / **`IncomingRuntimeMessage`**; **`PluginRegistry`** dispatches **`EventPacket`s**. Outbound: **`build_*_packet`** in **`command_bus.rs`**, **`send_json_packet` / `send_binary`**. No UI or file policy inside core.

### CLI (`src/`)

Thin shell: **`cli.rs`**, **`cmd_*.rs`**, **`plugin.rs`** (**`ConsolePlugin`**), **`state/`**, **`file_store.rs`**, **`file_flow.rs`**. **`LocalFileStore`** / **`LocalFileTransfer`** / **`LocalFileState`** — use **`move_to_incoming()`** / **`transition_to()`** structured errors when extending the state machine. **`main.rs::connect_with_console_plugin_with_retry`**: reconnect for **`run`** / **`chat`**; clears file state; **`reconnect_replays`** — see **`docs/当前实现说明.md`** §3.5.

### Security and auth (navigation)

- **`auth-service`** (planned): single identity/permission source; **`y2m-server`** stays a message bus.
- **`init`**: token validation via **`POST /auth/introspect`** (planned; **`InitPayload.token`** integration is the next security milestone).
- **RBAC** maps to **`EventType`s** (`text.send`, `json.send`, …); future: **`task.manage`**, **`shared_layer.lock`**, etc.
- **Device trust** for CLI: fingerprint MAC + IP + OS user; first login admin approval; trust token thereafter.
- **Session states**: **`active` / `suspended_readonly` / `revoked`** — **`revoked`** drops the socket immediately.
- **TLS**: **`wss://`** cross-machine; local **`ws://`** on **`127.0.0.1`**. See **`docs/加密验证方案.md`**.

### Cross-platform command execution

Use **`y2m_common::default_shell_program()`** / **`default_shell_arg()`** — **`cmd /C`** on Windows, **`sh -c`** elsewhere — for **`EventType::Command`**; do not hard-code a shell.

---

## Conventions worth preserving (codebase)

- **JSON wire format is camelCase** (`#[serde(rename_all = "camelCase")]`; **`ClientConfig`** on disk: `serverUrl`, `groupName`, …).
- **UTF-8 Chinese** in logs and user-visible CLI output is expected; do not strip it.
- **File control plane** may unicast or group-broadcast like other events; **binary chunks** only to legs that completed **`file_accept`**. Prefer explicit **`--to`** when the product flow is single-recipient.
- **All file state changes** go through **`LocalFileStore` / `LocalFileTransfer`** APIs; extend **`LocalFileState`** and transitions instead of parallel booleans.
- **Reconnect** must clear local file state before re-**`init`** (**`ConsoleState::clear_file_transfer_state()`** and any new **`ConsoleState`** fields you add).
