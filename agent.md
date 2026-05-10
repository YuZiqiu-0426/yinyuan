# agent.md（仓库指南）

本文件是隐元（YinYuan）仓库的**单一入口说明**：面向人类贡献者与 AI 编码助手（Claude Code、Cursor 等），汇总**工程约定**、**协作流程**、**目录与文档索引**、**构建与测试命令**、**架构与安全导航**。原根目录 `CONVENTIONS.md` 与 `CLAUDE.md` 已废止，内容并入本文件。

---

## 1. 文档角色与冲突优先级

当多处描述不一致时，按以下顺序解释：

1. **`docs/当前实现说明.md`** — 当前代码**事实基线**（已实现 / 未实现）。
2. **`docs/工作进度.md`** — 任务状态与优先级总表。
3. **`agent.md`**（本文件）— 如何在仓库内工作、命令入口、架构概览、工程规则。
4. **`docs/`** 下其他需求、设计、API、运维类文档。

若本文件与 **`docs/当前实现说明.md`** 冲突，**以《当前实现说明》为准**，并尽快修正 **`agent.md`**。

---

## 2. 语言与命名

- 面向人的说明文可使用**简体中文**（与现有文档一致）。
- **代码标识符、Git 提交说明、对外 API 名称**保持英文，除非已有既定例外。

---

## 3. 代码规模

1. **函数**：单函数不超过 **50 行**（含空行与注释）；超出则拆小函数或子模块。
2. **文件**：单源码文件不超过 **500 行**；接近上限时按职责拆分（如增加 `mod` 或并列模块）。

---

## 4. Git 工作流

3. **完成一项连贯任务后**：落地代码，再 **`git add`** 相关路径并 **`git commit`**。提交信息用完整句子说明**改了什么、为什么**。自动化助手在结束连贯任务时应实际提交，而非只打印命令让用户执行。

---

## 5. 运行时与工具

4. **tmux**：本地常驻服务、watch 等优先放在 **tmux** 会话/窗口中，避免无布局地散落在多个无关终端页签。
5. **前端包管理**：在 **`frontend-monorepo/`** 内仅使用 **pnpm**（`pnpm install`、`pnpm run <script>`）。日常不要引入 npm 或 Yarn，除非在本文件中有书面例外。

---

## 6. 项目约定

6. **仓库布局**：根目录以文档与协作为主；Rust 工作区在 **`y2m-rs/`**；前端应用在 **`frontend-monorepo/`**。
7. **Rust 命令**：所有 **`cargo`** 在 **`y2m-rs/`** 下执行（或使用 `--manifest-path y2m-rs/Cargo.toml`）。仓库根**不是** Rust workspace 根。
8. **实现描述**：以 **`docs/当前实现说明.md`** 为准描述「系统现在怎么做」。
9. **排期与下一步**：优先查阅 **`docs/工作进度.md`**。
10. **管理端与交互**：管理端 Web 为 **`frontend-monorepo/apps/y2-manage`**（Angular）。用户侧聊天 Web 已移除；用户交互为 **CLI-only**（**`y2m`**）。多 Agent 编排复用同一 WebSocket；业务消息可在 **`EventType::Json`** 之上承载 **`agent-collab`**（见 **`docs/agent-collab-protocol-v1.md`**）。
11. **行为基线**：路由、重连、文件传输、测试预期等与 **`docs/当前实现说明.md`** 一致；若有意改变产品行为，须先更新《当前实现说明》再改实现。

---

## 7. 流程约定

12. **改动前阅读**：非琐碎改动前，阅读 **`docs/当前实现说明.md`**、**`docs/工作进度.md`** 及本文件相关章节（布局、命令、测试）。
13. **以实装为准**：若设计稿或导航文与 **`docs/当前实现说明.md`** 矛盾，先对齐实现与《当前实现说明》。
14. **行为变更时的文档顺序**：
    1. `docs/当前实现说明.md`
    2. `docs/工作进度.md`
    3. **`agent.md`**（若命令、布局、测试分类或工程规则有变）
    4. 相关需求 / 设计 / API / Runbook
15. **测试**：连贯代码改动后，先跑**最小必要**校验，风险或范围大时再跑全 workspace。
16. **长驻进程**：尽量用 **tmux** 管理服务端与 watcher。

---

## 8. 本文件的引用关系

- **Cursor**：**`.cursor/rules/yinyuan-conventions.mdc`** 指向本文件，供辅助会话加载同一套规则。
- **安全与鉴权**：以 **`docs/`** 下专项为准（如 **`docs/加密验证方案.md`**、**`docs/权限矩阵与默认角色模板-v1.md`**）。若与旧需求（如 **`需求v1.md`**）冲突，**以安全类设计文档为准**。

---

## 9. 仓库布局与文档索引

Rust 工作区为 **`y2m-rs/`**。根目录常见设计/说明文档示例：

- **`需求v1.md`** — 原始需求（v1）。
- **`docs/当前实现说明.md`** — 实现状态；接新活前先读。
- **`docs/加密验证方案.md`** — 传输加密、设备信任、IP 白名单、密钥生命周期。
- **`docs/统一认证中心详细设计-v1.md`** — auth-service、JWT、CLI 设备因子等。
- **`docs/统一认证中心API定义-v1.md`** — 鉴权 API 与错误码。
- **`docs/权限矩阵与默认角色模板-v1.md`** — RBAC 与原子权限。
- **`docs/配置与密钥管理规范-v1.md`** — 环境变量密钥、JWT 轮换、安全基线。
- **`编译部署文档.md`** — 构建与部署笔记。
- **`使用手册.md`** — 终端用户说明。
- **`y2m-rs/docs/quickstart.md`** — 起服务与 CLI 的快速上手。

---

## 10. 构建与运行

```bash
cd y2m-rs
cargo build                      # 全 workspace 调试构建
cargo build --release            # release 产物在 target/release/
cargo build -p y2m               # 仅 CLI
cargo build -p y2m-server        # 仅服务端
```

产物说明：

- **`y2m-server`** — WebSocket 中继；默认 **`127.0.0.1:8080`**，可用环境变量 **`Y2M_SERVER_ADDR`** 覆盖；无命令行参数。
- **`y2m`** — 客户端 CLI：**`init | run | send | chat`**。

典型流程：起服务端 → **`y2m init --config alice.json ...`** → **`y2m chat --config alice.json`**（或被动模式 **`y2m run`**）。完整步骤见 **`y2m-rs/docs/quickstart.md`**。

---

## 11. 测试

```bash
cd y2m-rs
cargo check --workspace
cargo test --workspace
cargo test -p y2m
cargo test --test file_transfer_v3
cargo test --test cli_process_e2e -- --nocapture
```

**集成测试**（**`y2m-rs/tests/`**）两类：

1. **进程内**：如 `text_e2e.rs`、`json_e2e.rs`、`file_transfer_v3.rs` 等，在临时端口起 **`y2m_server::serve_with_listener_and_config`**，经 **`y2m_client_core`** 驱动；辅助逻辑见 **`tests/support/mod.rs`**。
2. **CLI 子进程 e2e**：如 `cli_process_e2e.rs`、`cli_file_*_e2e.rs`、`cli_reconnect_e2e.rs` 等，通过 **`tests/support/cli.rs`** 起真实二进制；较慢，部分用 **`serial_test`**。

除非必须覆盖 CLI 解析、stdin 驱动聊天或进程级重连，**优先写进程内测试**。

---

## 12. 架构（y2m-rs）

```text
y2m-rs/
  crates/common/       -> y2m-common       （协议类型等）
  crates/server/       -> y2m-server       （WebSocket 中继）
  crates/client-core/  -> y2m-client-core  （传输、组帧、插件分发）
  src/                 -> y2m              （CLI）
```

### 12.1 协议（`crates/common`）

协议版本 **`v3`**（**`PROTOCOL_VERSION`**）。控制面为 JSON **`Packet<T>`**，**`kind`** 含 **`init | init_ack | heartbeat | heartbeat_ack | event | ack | error`**。**`EventType`** 含 **`text`、`json`、`command`、`command_result`、`file_offer`、`file_accept`、`file_reject`、`file_complete`、`file_abort`**。文件字节走 **`BinaryChunkHeader`**，魔数 **`Y2MB`**，**`frame_type` = 1** 表示文件分片，与 JSON 共用同一 WebSocket。**`Endpoint { groupName, clientName }`** 寻址；**`Endpoint::server()`** 预留给服务端来源包。

### 12.2 服务端（`crates/server`）

**`ws.rs`**：axum WebSocket；**`init::handle_init`** 校验 init、拒绝同组重名、返回 **`init_ack`**。**`router.rs`**：**`text` / `json` / `command` / `command_result` / 全部 `file_*`** 支持**单播**（填写 **`target.clientName`**）或**组播**（省略 **`clientName`**，同组除发送者外，见 **`docs/当前实现说明.md`** §3.2）。**`session.rs`**：**`SessionStore`**。**`transfer.rs`**：**`TransferRegistry`**；**二进制分片**仅在对应接收腿 **`file_accept`** 后转发。心跳超时关连接并释放名称。

### 12.3 客户端核心（`crates/client-core`）

**`ClientCore::connect()`** 建链、发 **`init`**、等 **`init_ack`**、得到 **`ClientRuntime`**。入站经 **`runtime.dispatch_next()`** / **`IncomingRuntimeMessage`**；**`PluginRegistry`** 按 **`EventType`** 分发 **`EventPacket`**。出站经 **`command_bus.rs`** 的 **`build_*_packet`** 与 **`send_json_packet` / `send_binary`**。核心不含 UI 与文件策略。

### 12.4 CLI（`src/`）

薄封装：**`cli.rs`**、各 **`cmd_*.rs`**、**`plugin.rs`**（**`ConsolePlugin`**）、**`state/`**、**`file_store.rs`**、**`file_flow.rs`**。本地文件态统一 **`LocalFileStore` / `LocalFileTransfer` / `LocalFileState`**；扩展状态机时继续使用 **`move_to_incoming()`** / **`transition_to()`** 的结构化错误风格。**`main.rs::connect_with_console_plugin_with_retry`** 服务 **`run` / `chat`** 重连、清理文件态、**`reconnect_replays`**，见 **`docs/当前实现说明.md`** §3.5。

### 12.5 安全与鉴权（导航摘要）

- **`auth-service`**（规划中）：唯一身份与权限源；**`y2m-server`** 保持消息总线角色。
- **`init`**：规划在连接时经 **`POST /auth/introspect`** 校验 token；**`InitPayload.token`** 接入是下一安全里程碑。
- **RBAC** 与 **`EventType`** 对应（如 **`text.send`** 等）；后续可扩展 **`task.manage`**、**`shared_layer.lock`** 等。
- **CLI 设备信任**：指纹含 MAC、IP、OS 用户等；首登待审；后续信任 token。
- **会话态**：**`active` / `suspended_readonly` / `revoked`**；**`revoked`** 须立即断链。
- **TLS**：跨机 **`wss://`**；本机开发可用 **`ws://`** 访问 **`127.0.0.1`**。详见 **`docs/加密验证方案.md`**。

### 12.6 跨平台命令执行

对 **`EventType::Command`** 使用 **`y2m_common::default_shell_program()`** 与 **`default_shell_arg()`**（Windows 为 **`cmd /C`**，否则 **`sh -c`**），不要写死 shell。

---

## 13. 代码库中值得保持的约定

- **JSON 线路为 camelCase**（**`#[serde(rename_all = "camelCase")]`**；磁盘 **`ClientConfig`** 如 **`serverUrl`**、**`groupName`** 等）。
- **日志与用户可见 CLI 输出**允许 UTF-8 中文，不要随意剔除或强行 ASCII 化。
- **文件控制面**可与其它事件一样单播或组播；**二进制分片**仅发往已完成 **`file_accept`** 的接收腿。产品上要「单收方」时优先显式 **`--to`** / 目标。
- **文件状态变更**一律经 **`LocalFileStore` / `LocalFileTransfer`** API；新阶段用 **`LocalFileState`** 与迁移表扩展，避免平行布尔字段。
- **重连**须在再次 **`init`** 前清理本地文件相关状态（**`ConsoleState::clear_file_transfer_state()`** 路径）；若给 **`ConsoleState`** 增加新字段，重连逻辑中也要一并重置。
