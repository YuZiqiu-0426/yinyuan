# agent-collab 协议 v1（应用层）

> 版本：v1  
> 状态：规范骨架（与实现里程碑见 `docs/任务清单-v2.md` 模块四 AC-*、模块六 OR-*）  
> 变更记录：2026-05-10 初稿。

---

## 1. 文档目的与定位

`agent-collab-v1` 定义多 Agent 协作的**业务消息**形状与语义，运行在隐元 **y2m** 传输层之上。

- **不修改** `y2m-common` 中的核心 `EventType` 枚举；业务扩展优先使用 **`EventType::Json`** 承载本协议载荷。
- **编排状态机**（任务 PENDING→APPROVED 等）属于 **agent-orchestrator** 或等价进程，见 `docs/任务清单-v2.md` 模块六；**y2m-server** 保持消息中继与既有路由规则。

关联阅读：[多Agent前端协作开发方案.md](../多Agent前端协作开发方案.md)（流程与角色）、[spectrum-vixen-banshee.md](../spectrum-vixen-banshee.md)（y2m 能力对照）。

---

## 2. 与 y2m 传输层字段映射

协作方案中的逻辑结构 `SocketMessage` 映射到 y2m **`EventPacket`**（`kind = event`，`eventType = json`）如下。线路上均为 **camelCase**（与 `y2m-common` 一致）。

| 逻辑字段（协作方案） | y2m 字段 | 说明 |
|---------------------|----------|------|
| `id` | `requestId` | 包级请求 ID，用于 ack、去重与客户端聚合 |
| `type`（MessageType） | `payload.metadata.messageType` | 字符串，如 `task:assign` |
| `from` | `source.clientName`（及 `source.groupName`） | 服务端在路由后覆盖 `source`，发送方勿伪造 |
| `to` | `target.clientName` | 单播时必填；**组内广播时省略** `clientName`（仅 `groupName`） |
| `timestamp` | `payload.metadata.agentTimestamp`（可选） | 毫秒时间戳；若省略可用服务端到达顺序 |
| `payload` | `payload.content` | JSON 对象，业务体 |
| `replyTo` | `payload.metadata.replyTo` | 字符串，指向父级 `requestId` 或业务消息 id |

**广播语义**：`target.clientName` 为空且 `target.groupName` 为发送方所在组时，与同组 `text`/`json` 一致，向**除发送者外**的会话投递（以 `docs/当前实现说明.md` 为准）。

**原生事件复用**（推荐映射，减少重复造轮子）：

| messageType（逻辑） | 推荐 y2m 承载 |
|---------------------|---------------|
| `cmd:execute` 及执行结果 | `EventType::Command` / `EventType::CommandResult`（载荷见既有协议） |
| `file:offer` / 接受拒绝完成中止 | `EventType::FileOffer` / `FileAccept` / …（二进制块仍走既有分片头） |
| 其余 `task:*`、`shared-layer:*`、`subtask:*`、`chat`、纯协商类 | `EventType::Json` + 本表 `metadata.messageType` |

`heartbeat` 与 **Agent 在线身份** 仍以 y2m **`init` / `heartbeat`** 为准；逻辑上的 `agent:register` / `agent:status` 可作为 **Json** 消息类型在应用层补充能力声明（与任务清单 AG-02 对齐），**不替代** `init_ack`。

---

## 3. MessageType 枚举（字符串常量）

与 [多Agent前端协作开发方案.md](../多Agent前端协作开发方案.md) 第三节对齐，线路上统一为下列字符串（区分大小写，建议全小写+冒号）。

**任务**：`task:create`、`task:assign`、`task:accept`、`task:reject`、`task:progress`、`task:complete`、`task:verify`、`task:approve`、`task:reject-fix`、`task:reassign`。

**共享层**：`shared-layer:locked`、`shared-layer:verify`。

**子任务**：`subtask:assign`、`subtask:accept`、`subtask:complete`。

**资源（逻辑层）**：`file:offer`、`file:request`、`file:transfer` — 若已用原生 `file_*` 事件，则 Json 中可仅发**元数据同步**或省略，以编排器实现为准。

**命令（逻辑层）**：`cmd:execute`、`cmd:output`、`cmd:error` — 优先映射到原生 `command` / `command_result`。

**其它**：`chat`、`heartbeat`（应用层心跳若与 y2m 心跳并存，须在文档与实现中区分）、`agent:register`、`agent:status`。

---

## 4. 任务状态、阶段与消息对照

**任务状态**（编排器 OR-01，与协作方案 §3.2 一致）：

`PENDING` → `ASSIGNED` → `RUNNING` → `COMPLETED` → `VERIFYING` →（`APPROVED` | `NEEDS_FIX` / `REJECTED`）→ 修复后再次 `RUNNING` → … → `APPROVED`。

**TaskNode.stage**（协作方案 §4.1）：`vision` | `structure` | `implement` | `verify` | `fix`。

| stage | 典型 TaskStatus 片段 | 典型触发 / 消息（Json 或原生） |
|-------|----------------------|--------------------------------|
| vision | PENDING→RUNNING | 外部输入；可选 Json 标记解析完成 |
| structure | RUNNING→COMPLETED | `task:create` / 子任务拆分结果写入 content |
| implement | ASSIGNED→RUNNING→COMPLETED | `task:assign`、`subtask:assign`、`task:complete` |
| verify | COMPLETED→VERIFYING | `task:verify`；Merger 消费 `command_result` 或 Json |
| fix | NEEDS_FIX→RUNNING | `task:reject-fix`、`task:reassign`；`metadata.replyTo` 链到上一轮 requestId |

`APPROVED` 后可发 `shared-layer:verify` 或依赖 Git/CI 的验收，不在本表穷尽。

---

## 5. Json 事件载荷形状（约定）

`EventPacket` 中 `payload` 为 `EventPayload`：

- `eventType`: `json`
- `content`: **对象**，至少包含业务主键，例如 `{ "taskId": "page-login", "version": 1 }`（字段名 camelCase）
- `metadata`: **对象**，必须包含 `messageType`（字符串）；可选 `replyTo`、`agentTimestamp`、`schemaVersion: "agent-collab-v1"`

编排器与 Agent 客户端应对未知 `metadata` 键**前向兼容**（忽略不认识的键）。

---

## 6. 示例消息（逻辑等价 JSON）

以下为 **`content` + `metadata`** 示意；完整线包需带 y2m 顶层 `version`、`kind`、`requestId` 等，由 `y2m-client-core` 构造。

**6.1 创建任务（广播给组内编排监听者）**

```json
{
  "content": {
    "taskId": "page-login",
    "version": 1,
    "title": "Login page",
    "subtasks": []
  },
  "metadata": {
    "messageType": "task:create",
    "schemaVersion": "agent-collab-v1"
  }
}
```

**6.2 分配任务（单播至 `cursor-01`）**

```json
{
  "content": {
    "taskId": "page-login",
    "version": 1,
    "scope": "src/pages/LoginPage/",
    "promptRef": "tasks/task-login.json"
  },
  "metadata": {
    "messageType": "task:assign",
    "replyTo": "req-parent-uuid",
    "schemaVersion": "agent-collab-v1"
  }
}
```

**6.3 任务完成回传**

```json
{
  "content": {
    "taskId": "page-login",
    "version": 1,
    "summary": "Implemented layout; tests added.",
    "artifacts": ["worktree-ui/src/pages/LoginPage/"]
  },
  "metadata": {
    "messageType": "task:complete",
    "replyTo": "req-assign-uuid",
    "schemaVersion": "agent-collab-v1"
  }
}
```

---

## 7. 安全与权限（占位）

上线后由 `auth-service` 与 y2m `init` introspect 约束连接身份；扩展权限码见根目录 `agent.md`（`task.manage`、`shared_layer.lock` 等）。本 v1 规范不定义鉴权细节，见 `docs/权限矩阵与默认角色模板-v1.md` 后续版本。

---

## 8. 与里程碑关系

| 里程碑 | 说明 |
|--------|------|
| M4（README） | `agent-collab` schema、Agent 包装器、共享层锁定等，任务见 `docs/任务清单-v2.md` |
| AC-01～AC-06 | 本协议拆分任务；实现以代码与测试为准时可增补附录 `agent-collab-examples-v1.md` |
