# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Engineering conventions

- **`CONVENTIONS.md`** (repository root) — plain Markdown: per-function / per-file line limits, Git commit expectations after tasks. Applies to all tooling and contributors.

## Repository layout

The actual Rust workspace lives under `y2m-rs/`. All `cargo` commands must be run from that directory (or with `--manifest-path y2m-rs/Cargo.toml`). The repo root contains design docs and conventions, for example:

- `需求v1.md` — original requirements (v1).
- `docs/当前实现说明.md` — authoritative implementation status / what is done vs. pending. Read before picking up new work — it's kept updated as features land.
- `docs/加密验证方案.md` — transport encryption, device trust, IP whitelisting, and key lifecycle.
- `docs/统一认证中心详细设计-v1.md` — auth-service architecture, JWT session design, CLI device-factor auth.
- `docs/统一认证中心API定义-v1.md` — auth API contracts and error codes.
- `docs/权限矩阵与默认角色模板-v1.md` — RBAC role templates and atomic permissions.
- `docs/配置与密钥管理规范-v1.md` — env-var secrets, JWT key rotation, safety baselines.
- `编译部署文档.md` — build & deployment notes.
- `使用手册.md` — user manual.
- `y2m-rs/docs/quickstart.md` — end-user quickstart for running server and CLI clients.

## Build & run

```bash
cd y2m-rs
cargo build                      # debug build of whole workspace
cargo build --release            # release artifacts in target/release/
cargo build -p y2m               # client CLI only
cargo build -p y2m-server        # server only
```

Binaries after build:
- `y2m-server` — WebSocket relay server. Listens on `127.0.0.1:8080` by default; override with `Y2M_SERVER_ADDR` env var. No CLI flags.
- `y2m` — client CLI with subcommands `init | run | send | chat`.

Typical dev loop: start server in one terminal, then `y2m init --config alice.json ...` to generate a config, then `y2m chat --config alice.json` (or `y2m run ...` for passive mode). See `y2m-rs/docs/quickstart.md` for the full walkthrough.

## Tests

```bash
cd y2m-rs
cargo check --workspace
cargo test --workspace                       # all unit + integration tests
cargo test -p y2m                            # y2m binary's own unit tests (src/)
cargo test --test file_transfer_v3           # one integration test file
cargo test --test cli_process_e2e -- --nocapture   # spawns real y2m / y2m-server processes
```

Integration tests live in `y2m-rs/tests/`. There are two flavors:
1. **In-process tests** (e.g. `text_e2e.rs`, `json_e2e.rs`, `file_transfer_v3.rs`) spin up `y2m_server::serve_with_listener_and_config` on an ephemeral port and drive clients via `y2m_client_core` APIs directly. Helpers in `tests/support/mod.rs` (`spawn_server`, `connect_runtime`, `CaptureEventPlugin`, `CommandResponderPlugin`).
2. **CLI e2e tests** (`cli_process_e2e.rs`, `cli_file_*_e2e.rs`, `cli_reconnect_e2e.rs`) spawn the actual compiled `y2m` / `y2m-server` binaries via `tests/support/cli.rs` (`ProcessHandle`) and interact over real stdin/stdout. These are slower and require the workspace to build cleanly first. Some use `serial_test` because they bind ports / manage processes.

When adding tests, prefer the in-process style unless you specifically need to exercise CLI parsing, stdin-driven chat commands, or process-level reconnect behavior.

## Architecture

The system is three cooperating pieces that share one protocol crate:

```
y2m-rs/
  crates/common/         -> y2m-common       (protocol types, shared across server + client)
  crates/server/         -> y2m-server       (lib + bin: WebSocket relay)
  crates/client-core/    -> y2m-client-core  (lib: transport, framing, plugin dispatch)
  src/                   -> y2m              (bin: CLI that drives client-core)
```

### Protocol (`crates/common`)
Protocol version is `v3` (`PROTOCOL_VERSION` constant). All control-plane traffic is JSON `Packet<T>` with `kind` of `init | init_ack | heartbeat | heartbeat_ack | event | ack | error`. Events carry an `EventType` (`text`, `json`, `command`, `command_result`, `file_offer`, `file_accept`, `file_reject`, `file_complete`, `file_abort`). File bytes use a separate binary framing (`BinaryChunkHeader`, magic `Y2MB`, frame_type 1 = file chunk) on the same WebSocket — clients must handle both text and binary frames on a single socket. `Endpoint { groupName, clientName }` addresses everything. The `system/server` endpoint (`Endpoint::server()`) is reserved for server-originated packets.

### Server (`crates/server`)
`ws.rs` accepts WS connections via axum; each connection goes through `init::handle_init` (validates init, rejects duplicate `(group, client)`, returns `init_ack` with heartbeat/size limits). After init, `router.rs` routes events: `text`, `json`, `command`, `command_result`, and all `file_*` control-plane events support **unicast** (`target.clientName` set) or **group broadcast** (`target.clientName` omitted; same group, all sessions except sender—see `docs/当前实现说明.md` §3.2). `session.rs` (`SessionStore`) tracks live connections; `transfer.rs` (`TransferRegistry`) tracks file-transfer legs and **gates binary chunk forwarding** until each receiver leg has sent `file_accept`. Heartbeat timeout closes the connection with an error packet and frees the name for reconnect.

### Client core (`crates/client-core`)
`ClientCore::connect()` opens the WS, sends `init`, awaits `init_ack`, then hands back a `ClientRuntime`. Incoming messages (both JSON packets and binary chunks) arrive via `runtime.dispatch_next()` / `IncomingRuntimeMessage`; a `PluginRegistry` fans `EventPacket`s out to plugins registered for the matching `EventType`. Outgoing packets are built via the free `build_*_packet` helpers in `command_bus.rs` and sent through `ClientConnection::send_json_packet` / `send_binary`. The core is transport-plus-dispatch only — it deliberately has no UI or file-state logic.

### CLI (`src/`)
The `y2m` binary is a thin shell around `client-core`:
- `cli.rs` — clap definitions for `init / run / send / chat`.
- `cmd_init.rs`, `cmd_run.rs`, `cmd_send.rs`, `cmd_chat.rs` — one module per subcommand.
- `plugin.rs` (`ConsolePlugin`) — the single plugin that prints incoming text/json/command results and drives file transfer reactions. Currently all user-visible behavior funnels through this one plugin; splitting it into per-concern plugin crates is a planned P2 refactor (see `当前实现说明.md` §4 and §7.3).
- `state/` + `file_store.rs` + `file_flow.rs` — local file-transfer bookkeeping. All local state is consolidated into a single `LocalFileStore` holding `fileId -> LocalFileTransfer` with an explicit `LocalFileState` phase enum. State transitions go through `LocalFileTransfer::move_to_incoming()` / `transition_to()`, which return structured errors (`UnexpectedState`, `InvalidTransition`) rather than booleans — **preserve this structured-error style when extending the state machine**.
- `main.rs::connect_with_console_plugin_with_retry` — the reconnect loop used by `run` and `chat`. On reconnect it clears pending/incoming/outgoing file state and replays user-facing "stale/failed" messages via `reconnect_replays`; see `当前实现说明.md` §3.5 for the expected user-visible behavior.

### Security and auth architecture
The repository includes a complete unified-auth design under `docs/`. Key points for navigation:
- **Auth service** (`auth-service`, Rust/Axum, planned) is the single identity/permission source. `y2m-server` remains a pure message bus and delegates auth decisions to it.
- **Token validation** in `y2m-server` happens at `init::handle_init` via `POST /auth/introspect`. The `token` field in `InitPayload` is currently ignored; integrating the auth check is the next security milestone.
- **RBAC permission codes** map to `EventType`s: `text.send`, `json.send`, `command.send`, `file.send` (and corresponding `*.recv`). Multi-Agent orchestration will extend these with `task.manage`, `shared_layer.lock`, etc.
- **Device trust** for CLI clients: each `y2m` instance registers as a `device` with fingerprint = MAC + IP + OS user. First login requires admin approval; subsequent logins use a trust token.
- **Session state machine**: `active` / `suspended_readonly` / `revoked`. `revoked` must immediately drop the WebSocket connection; `suspended_readonly` allows receive-only.
- **TLS**: `wss://` for cross-machine deployments; local dev may use `ws://` on `127.0.0.1`. See `docs/加密验证方案.md` §3 for TLS 1.3 + mTLS guidance.

### Cross-platform command execution
`y2m-common` exposes `default_shell_program()` / `default_shell_arg()` returning `cmd /C` on Windows and `sh -c` elsewhere. Use these when adding anything that shells out for `EventType::Command` — don't hard-code a shell.

## Conventions worth preserving

- **JSON wire format is camelCase.** All protocol structs use `#[serde(rename_all = "camelCase")]`; `ClientConfig` on disk is camelCase too (`serverUrl`, `groupName`, `downloadDir`, ...).
- **UTF-8 Chinese strings are expected in logs and user-facing output** (status lines, `/files` output, reconnect replays). Don't strip or ASCII-fy them.
- **File transfer control plane** (`file_offer`, etc.) may be unicast or group broadcast like other events; **binary file chunks** are only forwarded to receiver legs that have completed `file_accept`. Prefer explicit `--to` / target when the product flow should be single-recipient.
- **All file state changes go through `LocalFileStore` / `LocalFileTransfer` methods**, not by reaching into fields directly. When adding a new phase or transition, extend `LocalFileState` + the transition table rather than adding parallel booleans.
- **Reconnect must clear local file state before re-`init`** (the existing `ConsoleState::clear_file_transfer_state()` path). New local state added to `ConsoleState` needs to be reset there too, otherwise reconnect will leave zombie entries.
