# y2m-rs 胜任多Agent前端协作Socket方案评估

## 一、结论

**y2m-rs 的传输层完全胜任，应用层协议需要适配扩展。**

y2m-rs 提供了一个成熟的 WebSocket 消息总线（心跳、注册、单播/广播、文件分块传输、ACK机制），方案中第3节 Socket消息协议 的通信基础设施可以用 y2m-rs 承载，但需要在其之上定义一层业务消息规范。

---

## 二、现有能力对照

| 方案需求 | y2m-rs 现状 | 匹配度 |
|---------|------------|-------|
| WebSocket 长连接 | axum + tokio_tungstenite 双端实现 | 完全匹配 |
| Agent 注册 (agent:register) | Init 包 (group_name + client_name + capabilities) | 完全匹配 |
| 心跳 (heartbeat) | Heartbeat / HeartbeatAck + 超时清理任务 | 完全匹配 |
| 单播/广播 | SessionStore.resolve_unicast / resolve_broadcast | 完全匹配 |
| 文件传输 | FileOffer -> FileAccept -> 二进制分块 (BinaryChunkHeader) | 完全匹配 |
| 命令执行 (cmd:execute) | `EventType::Command`；协议含 `command_plugin` 能力位，**当前 relay 未按该位做路由强制**（见 `docs/任务清单-v2.md` SV-04 等） | 协议匹配 / 强制待接入 |
| JSON 业务消息 | `EventType::Json`；协议含 `json_plugin` 能力位，**当前 relay 未按该位做路由强制** | 协议匹配 / 强制待接入 |
| 任务管理消息 (task:create 等) | 无内置类型，需通过 EventType::Json 的 payload/metadata 承载 | 需适配 |
| 任务状态机 | 无，属于纯应用层逻辑 | 需新增 |
| Agent 能力矩阵 & 调度 | 无，属于纯应用层逻辑 | 需新增 |
| replyTo 消息关联 | 无直接字段，但 request_id 可复用 | 需约定 |

---

## 三、协议映射建议

方案中的 SocketMessage 可以映射为 y2m 的 EventPacket（`EventType::Json`）。线路上 JSON 字段为 **camelCase**（如 `requestId`、`clientName`）。

- id -> `requestId`（包级）
- type -> `payload.metadata.messageType`（应用层约定）
- 逻辑上的 from -> **`source`**：`source` **由服务端在路由后写入**，与发送端 `init` 身份一致；客户端不应依赖自拟字段冒充发送方
- to -> `target.clientName`（组播时省略 `clientName`，仅组名）
- timestamp -> 包体顶层 `timestamp`（或放入 `metadata` 作补充，见 `docs/agent-collab-protocol-v1.md`）
- payload -> `payload.content`
- replyTo -> `payload.metadata.replyTo`

### 推荐做法（最小侵入）

不改动 y2m 核心协议（保持与其他 y2m 客户端兼容），在 EventType::Json 的 content / metadata 中定义业务层 schema。

---

## 四、需要调整的内容

### 1. 业务层协议扩展（纯约定，不改 y2m 核心）
- 定义 agent-collab 消息 schema（基于 EventType::Json）
- 映射所有 MessageType 到 JSON 消息体
- 约定 replyTo 放在 metadata.replyTo 中

### 2. 服务端新增应用层模块（在 y2m-server 之上或旁边）
- TaskStore：内存中的任务状态机（PENDING -> ASSIGNED -> RUNNING -> ...）
- AgentRegistry：能力矩阵 + 负载跟踪
- Scheduler：基于 capabilityMatch / load / successRate 的评分算法
- SharedLayerManager：共享层锁定 + checksum 校验

建议：不要塞进 y2m-server 核心，而是作为独立服务（如 agent-orchestrator）连接 y2m-server，或作为 y2m-server 的一个 crate 插件。y2m-server 保持为纯消息总线。

### 3. Agent 进程包装器
方案中的 AgentProcess 需要基于 y2m CLI 实现一个长期运行模式：

现有: y2m run --server ws://host:port --group frontend --name claude-router
扩展: y2m agent --role router --capabilities react,typescript,architecture
      -> 长期运行，循环接收 Event，生成上下文文件，调用本地 CLI，发送结果

y2m CLI 已有 cmd_run（交互式 REPL）和 cmd_chat，需要新增一个 cmd_agent 子命令：
- 连接后发送 Init
- 循环监听 Event（过滤 EventType::Json 中 messageType 为任务相关的）
- 收到任务后：生成上下文文件 -> spawn 本地 AI CLI -> 收集输出 -> 发送 task:complete
- 维护内存中的 ContextManager

### 4. 分组策略
方案中的多 Agent 可以在同一个 y2m group 中（如 group_name: "frontend"），通过 client_name 区分各 Agent（claude-router, kimi-parser, cursor-coder...）。

y2m 的广播机制天然支持 group 内广播（发送方除外），正好对应方案中的 "广播时省略 to"。

---

## 五、如果后续要实施，建议的推进路线

**Phase 1：协议适配（1-2天）**
- 定义 agent-collab-v1 JSON schema（映射所有 MessageType）
- 用 y2m CLI 手动测试两端收发

**Phase 2：Agent 包装器（2-3天）**
- 新增 y2m agent 子命令（长期运行模式）
- 实现上下文文件生成 + CLI 调用封装
- 支持修复任务的上下文注入

**Phase 3：编排服务（3-5天）**
- 开发 agent-orchestrator（可独立进程，也可嵌入 y2m-server）
- 实现 TaskStore、AgentRegistry、Scheduler
- 对接 y2m-server WebSocket

**Phase 4：共享层机制（1-2天）**
- 共享层预实现流程（Claude 生成 -> 锁定 -> 分发约束）
- checksum 校验

---

## 六、最终判断

| 层面 | 是否胜任 | 说明 |
|-----|---------|------|
| 传输层 | 完全胜任 | 心跳、WS、路由、文件传输都成熟 |
| 会话管理 | 完全胜任 | group/client 模型天然匹配 Agent 分组 |
| 消息协议 | 需适配层 | 通过 EventType::Json 承载业务类型，无需改核心 |
| 任务状态机 | 需新建 | 纯应用层，建议另起 orchestrator |
| Agent 调度 | 需新建 | 纯应用层 |
| 共享层锁定 | 需新建 | 纯应用层 |

**一句话：y2m-rs 是一个很好的底层消息总线，可以直接复用；多 Agent 协作的业务逻辑需要在其之上新建一层编排服务。**
