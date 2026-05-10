# 多Agent前端协作开发方案

> 基于Socket通信的异构AI Agent协作架构设计

---

## 一、参与Agent能力画像

> 下表中各工具的「核心模型」版本号为撰写时市场概况，**仅供角色选型参考**；实施与采购以各厂商当时文档为准。

| 工具 | 核心模型 | 优势 | 短板 | 最佳角色 |
|------|---------|------|------|---------|
| **Claude** | Claude 4 Sonnet/Opus | 架构设计、复杂推理、长上下文理解、代码审查 | 速度较慢、成本高 | **架构师 / Router / Merger / 共享层预实现** |
| **Kimi** | Kimi K2.6 | 超长上下文（200万token）、中文理解、多模态 | 代码生成一致性略逊于Claude | **需求分析 / 任务结构化 / 长文档处理** |
| **OpenCode** | DeepSeek V4 | 代码生成速度快、数学/逻辑强、开源生态 | 创意较弱、可能过度工程化 | **核心模块执行 / 复杂逻辑实现** |
| **Cursor** | GPT-4o / Claude 3.7 | IDE集成度最高、实时补全、重构能力强 | 上下文窗口相对小、容易被"带偏" | **UI实现 / 快速原型 / 交互代码** |
| **Codex** | Codex (o3/o4-mini) | 执行速度极快、批量处理、GitHub原生集成 | 深度推理弱、需要明确指令 | **批量任务 / 测试生成 / 文档补全** |
| **Gemini** | Gemini 2.5 Pro | 多模态（图/视频/音频）、Google生态、超大上下文 | 代码风格不稳定、幻觉率较高 | **设计稿解析 / 视觉提示词生成** |

---

## 二、整体协作架构

### 2.1 分层流水线

```
┌─────────────────────────────────────────┐
│  Layer 1: 决策层 (Strategic)            │
│  ┌─────────┐  ┌─────────┐              │
│  │ Claude  │  │  Kimi   │  ← 双Router备份│
│  │ 主路由  │  │ 副路由   │  ← Kimi 200万上下文│
│  │架构设计 │  │需求拆解 │    适合处理超大PRD │
│  └────┬────┘  └────┬────┘              │
│       └─────────────┘                    │
│              ↓ Task Package (JSON)       │
├─────────────────────────────────────────┤
│  Layer 2: 执行层 (Tactical) - 并行       │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │ Gemini  │ │OpenCode │ │ Cursor  │   │
│  │ 视觉解析 │ │核心模块 │ │ UI/交互 │   │
│  │+提示词   │ │(DeepSeek│ │(快速原型│   │
│  │ 生成    │ │  V4)   │ │        │   │
│  └────┬────┘ └────┬────┘ └────┬────┘   │
│       └────────────┴───────────┘        │
│              ↓ 代码 + 测试                │
├─────────────────────────────────────────┤
│  Layer 3: 验证层 (Validation)            │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐ │
│  │ Claude  │  │  Codex  │  │  Kimi   │ │
│  │ Merger  │  │批量测试  │  │文档审查  │ │
│  │合并验收 │  │文档补全 │  │长文本校验│ │
│  └─────────┘  └─────────┘  └─────────┘ │
└─────────────────────────────────────────┘
```

### 2.2 前端专项架构（UI设计稿+接口文档已就绪）

```
用户: 实现登录页UI
  ↓
Gemini: 解析设计稿 designs/login.png
  → 输出: vision/login.json（结构化视觉提示词）
  ↓
Kimi: 读取 vision/login.json + docs/api-doc.md
  → 输出: tasks/task-login.json（可执行任务包）
  ↓
Claude: 分析任务包，识别共享层
  → 预实现: shared/（types, api, hooks, ui组件, utils）
  → 锁定: shared/ v1.0
  ↓
并行分发页面实现（带共享层约束）:
  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
  │  OpenCode   │ │   Cursor    │ │    Codex    │
  │ LoginForm   │ │ LoginPage   │ │ 测试生成    │
  │ (复杂逻辑)  │ │ (页面组装)  │ │ (批量)     │
  └─────────────┘ └─────────────┘ └─────────────┘
  ↓
Claude: 合并 + 验收 + 优化
  → 检查: 共享层完整性、类型一致性、响应式适配
  → 输出: 最终代码 + 验收报告
```

---

## 二点五、与隐元（YinYuan）实现对齐

本协作方案描述的是**角色分工与流程**；在隐元仓库中落地时，下列约定与代码、任务清单一致。

### 传输层（默认 y2m）

- **默认承载**为隐元 **`y2m-server` WebSocket** + v3 JSON 控制面；多 Agent 同组内通信可复用 **`EventType::Json`** 承载业务消息，字段映射与消息类型见 [`../orchestration/agent-collab-protocol-v1.md`](../orchestration/agent-collab-protocol-v1.md)。
- 下文第七节中的 `socket.emit` / `io('ws://…')` 表示**与 Socket.IO 在语义上等价**的「发布—订阅、房间、事件名」模型；**实现上不强制**使用 Socket.IO，推荐使用 `y2m-client-core` 或后续 `y2m agent` 子命令连接同一 relay。
- 文件字节、远程命令执行等仍优先映射到 y2m 原生 **`file_*` / `command` / `command_result`**，避免重复定义二进制帧与 shell 语义。能力对照见 [`./spectrum-vixen-banshee.md`](./spectrum-vixen-banshee.md)。

### 编排层（agent-orchestrator）

- **任务状态机、调度评分、共享层 checksum 校验**等放在 **agent-orchestrator**（独立进程或独立 crate），经 WebSocket 与 `y2m-server` 收发消息；**不把**状态机硬塞进 `y2m-server` 核心路由。详见 [`../product/任务清单-v2.md`](../product/任务清单-v2.md) 模块六（OR-*）与 OR-09。

### 双 Router（Claude + Kimi）决策规则

- **可执行任务包**（如 `tasks/*.json`）须有**唯一终稿发布者**，避免两份冲突清单并行流入执行层。推荐约定二选一（团队可固化其一）：
  - **A**：Kimi 产出结构草稿 → **Claude 审核合并后**写入仓库终稿路径，再触发 `task:assign`；或
  - **B**：仅 **Claude** 写入可分配任务包，Kimi 输出仅作附件上下文，不直接作为调度真源。
- 发生冲突时以 **Git 上已合并的主线** 与 orchestrator 内 **版本号 + rootId** 为准。

### 仓库路径与 Windows / monorepo

- 管理端 Web 应用目录为 **`frontend-monorepo/apps/y2-manage`**（Angular）；包管理使用 **pnpm**，与仓库根 [`../../agent.md`](../../agent.md) 中 pnpm 约定一致。
- 第八节目录与 `start-agents.sh` 为 **Unix 示意**；在 **Windows** 上需改用 PowerShell 或 WSL、注意路径分隔符与后台进程启动方式；多 worktree 与 monorepo 并存时，避免多个应用根各自重复安装依赖，建议以 **monorepo 根** 为唯一 `pnpm install` 入口。

---

## 三、通信机制

### 3.1 Socket消息协议

> **实现提示**：线路上推荐使用 y2m `EventPacket`（`json` 事件 + `metadata.messageType`），见 `docs/orchestration/agent-collab-protocol-v1.md`；下表为与实现对齐的逻辑类型定义。

```typescript
// 基础消息结构
interface SocketMessage {
  id: string;           // 消息唯一ID
  type: MessageType;    // 消息类型
  from: string;         // 发送方Agent ID
  to?: string;          // 接收方Agent ID（广播时省略）
  timestamp: number;    // 发送时间戳
  payload: unknown;     // 具体载荷
  replyTo?: string;     // 回复哪条消息
}

// 消息类型枚举
enum MessageType {
  // 任务管理
  TASK_CREATE = 'task:create',
  TASK_ASSIGN = 'task:assign',
  TASK_ACCEPT = 'task:accept',
  TASK_REJECT = 'task:reject',
  TASK_PROGRESS = 'task:progress',
  TASK_COMPLETE = 'task:complete',
  TASK_VERIFY = 'task:verify',
  TASK_APPROVE = 'task:approve',
  TASK_REJECT_FIX = 'task:reject-fix',
  TASK_REASSIGN = 'task:reassign',      // 迭代修复时重新分配

  // 共享层管理
  SHARED_LAYER_LOCKED = 'shared-layer:locked',
  SHARED_LAYER_VERIFY = 'shared-layer:verify',

  // 子任务并行
  SUBTASK_ASSIGN = 'subtask:assign',
  SUBTASK_ACCEPT = 'subtask:accept',
  SUBTASK_COMPLETE = 'subtask:complete',

  // 资源传输
  FILE_OFFER = 'file:offer',
  FILE_REQUEST = 'file:request',
  FILE_TRANSFER = 'file:transfer',

  // 命令执行
  CMD_EXECUTE = 'cmd:execute',
  CMD_OUTPUT = 'cmd:output',
  CMD_ERROR = 'cmd:error',

  // 对话协商
  CHAT = 'chat',

  // 心跳与发现
  HEARTBEAT = 'heartbeat',
  AGENT_REGISTER = 'agent:register',
  AGENT_STATUS = 'agent:status',
}
```

### 3.2 任务状态流转

```
PENDING → ASSIGNED → RUNNING → COMPLETED → VERIFYING
                                      ↓
                              NEEDS_FIX ← REJECTED
                                      ↓
                                   RUNNING (v2)
                                      ↓
                                   COMPLETED
                                      ↓
                                   APPROVED (锁定)
```

---

## 四、任务树与迭代机制

### 4.1 任务树结构

```typescript
interface TaskNode {
  id: string;
  version: number;               // 迭代版本，从1开始
  parentId?: string;
  rootId: string;

  stage: TaskStage;              // vision | structure | implement | verify | fix
  status: TaskStatus;            // pending | running | completed | failed | needs-fix | approved

  executions: ExecutionRecord[];  // 每次执行的记录
  iterations: IterationRecord[]; // 每次修改的历史

  dependencies: string[];        // 依赖的其他任务
  dependents: string[];         // 依赖本任务的其他任务

  context: TaskContext;          // 任务专属上下文
  sharedContext: string;         // 引用共享上下文ID
}

interface IterationRecord {
  version: number;
  triggeredBy: string;          // 验收不通过 / 需求变更 / 依赖更新
  changes: string;
  previousOutput: string;
  newOutput: string;
  diff: string;
}
```

### 4.2 迭代触发场景

| 触发条件 | 处理方式 |
|---------|---------|
| **验收不通过** | 记录问题 → 版本+1 → 原Agent修复（带历史上下文） |
| **需求变更** | 影响分析 → 级联触发相关任务迭代 |
| **依赖更新** | 下游任务自动标记为需更新 |
| **共享层升级** | 所有引用该版本的页面任务提示更新 |

---

## 五、并行实现机制

### 5.1 任务拆分（Structure阶段）

```json
{
  "taskId": "page-login",
  "page": "LoginPage",
  "subTasks": [
    {
      "subTaskId": "login-form",
      "type": "component",
      "scope": "src/components/LoginForm/",
      "complexity": 5,
      "requiredCapabilities": ["react", "typescript", "form-handling"],
      "sharedDeps": ["Button", "Input", "ErrorToast"]
    },
    {
      "subTaskId": "oauth-buttons",
      "type": "component",
      "scope": "src/components/OAuthButtons/",
      "complexity": 3,
      "requiredCapabilities": ["react", "oauth"],
      "sharedDeps": ["Button", "Icon"]
    },
    {
      "subTaskId": "login-page",
      "type": "page",
      "scope": "src/pages/LoginPage/",
      "complexity": 4,
      "requiredCapabilities": ["react", "layout", "responsive"],
      "sharedDeps": ["LoginForm", "OAuthButtons", "Header"]
    }
  ],
  "sharedComponents": ["Button", "Input", "ErrorToast", "Icon", "Header"]
}
```

### 5.2 动态调度算法

```typescript
// Agent评分维度
const score = 
  capabilityMatch * 0.4 +      // 能力匹配度
  (1 - currentLoad) * 0.2 +     // 负载评分（越低越好）
  performance.successRate * 0.2 + // 历史成功率
  complexityFit * 0.2;            // 复杂度适配

// Agent能力矩阵
const agents = [
  { id: 'opencode-01', capabilities: ['deep-reasoning', 'algorithm'], maxConcurrent: 2 },
  { id: 'cursor-01', capabilities: ['react', 'typescript', 'tailwind'], maxConcurrent: 3 },
  { id: 'codex-01', capabilities: ['bulk-generation', 'test-generation'], maxConcurrent: 5 }
];
```

### 5.3 拓扑排序（依赖优先）

```
共享组件（Button/Input/Modal）→ 页面专属组件 → 页面组装
     ↓
  无依赖的先执行，有依赖的等待完成后启动
```

---

## 六、共享层预实现机制

### 6.1 Claude预实现范围

```bash
shared/
├── types/
│   ├── index.ts          # 统一导出
│   ├── auth.ts           # AuthResponse, LoginRequest, User
│   ├── api.ts            # ApiError, ApiResponse<T>
│   └── dashboard.ts      # DashboardStats
├── api/
│   ├── index.ts          # axios实例配置
│   ├── auth-api.ts       # login(), logout(), refreshToken()
│   ├── user-api.ts       # getProfile(), updateProfile()
│   └── dashboard-api.ts  # getStats()
├── hooks/
│   ├── useAuth.ts        # 认证状态+自动刷新
│   ├── useApi.ts         # API调用封装
│   ├── useForm.ts        # 表单验证+提交
│   └── usePermission.ts  # 权限检查
├── components/
│   └── ui/
│       ├── Button.tsx      # 变体: primary/secondary/ghost
│       ├── Input.tsx       # 支持error状态、图标
│       ├── Modal.tsx       # 支持确认/取消、动画
│       ├── LoadingSpinner.tsx
│       ├── ErrorToast.tsx
│       └── EmptyState.tsx
├── utils/
│   ├── format.ts         # formatDate, formatNumber
│   ├── validate.ts       # validateEmail, validatePhone
│   └── error-handler.ts  # handleApiError
└── theme/
    └── tokens.ts         # 从tokens.json生成的TS常量
```

### 6.2 共享层锁定

```typescript
interface SharedLayerLock {
  taskId: string;
  version: number;
  path: string;
  checksum: string;      // 文件哈希，防止篡改
  lockedBy: string;
  lockTime: number;
}

// 页面Agent验收时验证
function verifySharedLayer(taskId: string, version: number): boolean {
  const lock = sharedLocks.get(`${taskId}@${version}`);
  const currentChecksum = computeDirectoryHash(lock.path);
  return currentChecksum === lock.checksum;
}
```

### 6.3 页面Agent约束

```markdown
## ⚠️ 重要约束

### 共享层（只读，禁止修改）
```typescript
// ✅ 正确：从共享层导入
import { Button, Input, ErrorToast } from '@/shared/components/ui';
import { useAuth } from '@/shared/hooks/useAuth';
import { authApi } from '@/shared/api/auth-api';
import { LoginRequest } from '@/shared/types/auth';

// ❌ 错误：自己实现Button或修改共享层
```

### 验收标准
1. 代码中无 `shared/` 目录的写操作
2. 无重复实现共享组件
3. 所有API调用必须通过 `shared/api/`
4. 所有类型从 `shared/types/` 导入
5. 通过TypeScript严格检查
```

---

## 七、各Agent调用方式

### 7.1 CLI上下文保持机制

| 工具 | 无状态调用 | 有状态调用 | 上下文保留 |
|------|-----------|-----------|-----------|
| **Claude** | `claude --print "prompt"` | `claude` 交互式 / `-r`恢复 | 自动保存会话 |
| **Kimi** | `kimi --print --afk "prompt"` | `kimi` 交互式 / `-r`恢复 | 自动保存会话 |
| **OpenCode** | `opencode --print "prompt"` | `opencode` 交互式 | 自动保存会话 |
| **Cursor** | `cursor-agent -p --force "prompt"` | 无原生交互式 | 需外部管理 |
| **Codex** | `codex --approval-mode full-auto` | `codex` 交互式 | 自动保存会话 |
| **Gemini** | `gemini --prompt "..."` | API模式 | 需通过API session参数 |

### 7.2 Agent进程长期运行模式

```typescript
// Agent进程核心逻辑
class AgentProcess {
  private context: ContextManager;    // 内存中的上下文
  private socket: Socket;

  constructor(agentId: string, role: string) {
    this.context = new ContextManager(agentId);
    this.socket = io('ws://central-server:3000');
    this.register();
  }

  // 1. 连接Socket服务器
  register() {
    this.socket.emit('agent:register', {
      agentId: this.agentId,
      role: this.role,
      capabilities: this.capabilities,
      status: 'idle'
    });
  }

  // 2. 监听任务分配
  onTaskAssign(msg: SocketMessage) {
    // 接受任务
    // 生成带上下文的任务文件
    const ctxFile = this.context.generateContextFile(msg.payload.prompt);

    // 调用本地CLI
    const result = this.callCLI(ctxFile);

    // 记录执行历史
    this.context.recordExecution(msg.payload.taskId, result);

    // 返回结果
    this.socket.emit('task:complete', { ... });
  }

  // 3. 调用CLI时注入上下文
  callCLI(ctxFile: string): Promise<string> {
    return spawn('claude', [
      '--print',
      '--allowedTools', 'Read,Write,Bash',
      `读取 ${ctxFile}，执行其中的任务，返回结果。`
    ]);
  }
}
```

### 7.3 修复时的上下文注入

```typescript
// 生成修复专用上下文
function generateFixContextFile(params: FixContextParams): string {
  return `
# 修复任务 ${params.taskId} - 第 ${params.version} 版

## ⚠️ 重要：这是第 ${params.previousAttempts + 1} 次尝试
之前的尝试存在问题，请仔细阅读修复要求，避免重复错误。

## 历史记录
${params.history}

## 上一版代码（需要修改的部分）
${params.previousFiles.map(f => `
### ${f.path}
\`\`\`
${f.content}
\`\`\`
`).join('
')}

## 修复要求（必须严格遵守）
${params.fixInstructions}

## 约束
1. 只修改有问题的部分，不要重写整个文件
2. 保持与未修改部分的风格一致
3. 修改后必须通过类型检查和lint
4. 如果无法修复，明确说明原因
`;
}
```

---

## 八、单机器部署方案

### 8.1 目录结构

```bash
~/frontend-project/
├── .orchestrator/
│   ├── contracts/              # 共享契约（所有Agent只读）
│   │   ├── api-contract.ts
│   │   └── theme/
│   │       └── tokens.json
│   ├── vision/                 # Gemini输出
│   ├── tasks/                  # Kimi输出
│   └── results/
│
├── main/                         # Claude工作区（最终产物）
│   ├── src/
│   │   └── shared/             # Claude预实现的共享层
│   └── .claude/CLAUDE.md
│
├── worktree-vision/              # Gemini工作区
│   ├── designs/
│   └── .gemini/
│
├── worktree-design/              # Kimi工作区
│   ├── docs/
│   └── .kimi/AGENTS.md
│
├── worktree-core/                # OpenCode工作区
│   └── .opencode/AGENTS.md
│
└── worktree-ui/                  # Cursor工作区
    └── .cursorrules
```

### 8.2 启动脚本

```bash
#!/bin/bash
# start-agents.sh

# 1. 启动Socket服务器
node server.js &
sleep 2

# 2. 启动各Agent（长期运行进程）
node agents/claude-router.js &
node agents/gemini-vision.js &
node agents/kimi-parser.js &
node agents/opencode-coder.js &
node agents/cursor-coder.js &
node agents/codex-tester.js &

wait
```

---

## 九、关键限制与对策

| 限制 | 对策 |
|------|------|
| **CLI无状态** | Agent进程长期运行，维护内存上下文 |
| **上下文太大** | 只保留最近20个任务历史，项目规范常驻 |
| **多任务并发** | 每个任务生成独立的上下文文件 |
| **上下文持久化** | 定期保存到 `/tmp/agent-context/`，进程重启可恢复 |
| **跨Agent共享** | 通过Socket传输关键上下文，或共享文件系统 |
| **Cursor CLI hang问题** | 用 `timeout 600` 包裹，或改用OpenAPI调用 |
| **Cursor无法pipe输入** | 用 `$(cat prompt.txt)` 或文件传参 |
| **多Agent同时读写contracts** | contracts目录设为只读共享，Agent只读不写 |
| **Kimi子Agent审批风暴** | 在Task描述中加防风暴指导，限制max_steps |
| **共享层被篡改** | 文件哈希校验，合并时验证完整性 |
| **任务边界不清** | Kimi在任务包中明确定义共享组件，Cursor优先实现共享组件库 |

---

## 十、最小可行版本（MVP）

### 10.1 最简组合（推荐启动）

```
Claude (Router+Merger+共享层) + Kimi (需求/任务结构化) + OpenCode (核心代码) + Cursor (UI)
```

**理由：**
- Claude和Kimi都有超长上下文，适合做大脑
- OpenCode和Cursor都有成熟的headless模式，适合做手
- 四个工具的配置目录天然隔离（`~/.claude/`、`~/.kimi/`、`~/.opencode/`、`~/.cursor/`）

### 10.2 渐进式扩展

| 阶段 | 加入Agent | 目标 |
|------|----------|------|
| Phase 1 | Claude + Cursor | 验证核心协作流程 |
| Phase 2 | + Kimi | 解决长文档理解问题 |
| Phase 3 | + OpenCode | 复杂逻辑并行实现 |
| Phase 4 | + Codex | 批量测试/文档补全 |
| Phase 5 | + Gemini | 设计稿自动解析 |

---

## 十一、完整迭代流程示例

```
Round 1: 初始实现
  Gemini → 解析设计稿 → vision/login-v1.json ✅
  Kimi → 结构化任务 → tasks/task-login.json ✅
  Claude → 预实现共享层 → shared/ v1.0 ✅
  OpenCode → 实现LoginForm → 提交 ✅
  Cursor → 组装LoginPage → 提交 ✅
  Claude → 验收 → ❌ 发现问题：
    - LoginForm.tsx:24 硬编码颜色 #ff0000
    - 未处理429错误

Round 2: 自动修复
  Cursor → 接收修复任务 (login-page-v2)
    上下文包含：
      - 上一版代码 (login-page-v1)
      - 具体问题列表
      - 历史记录 (第2次尝试)
    → 修复后提交 ✅
  Claude → 验收 → ❌ 还有问题：
    - 429处理逻辑不对，应该节流而不是直接报错

Round 3: 再次修复
  Cursor → 接收修复任务 (login-page-v3)
    上下文包含：
      - 第1版代码 (v1)
      - 第2版代码 (v2)
      - 两次的问题列表
      - 明确指示：429应该节流重试
    → 修复后提交 ✅
  Claude → 验收 → ✅ 通过
    → 锁定 login-page-v3 版本
    → 通知所有Agent：该任务已完成

用户变更: 登录页颜色要改，改成蓝色主题

Round 4: 需求变更
  Claude → 分析影响范围
    - vision阶段：需要重新提取颜色 → 影响Gemini
    - implement阶段：需要改样式 → 影响Cursor
    - structure阶段：不受影响 → Kimi不需要重做

  触发迭代：
    Gemini → 重新解析 (login-vision-v2) → 提取新颜色
    Cursor → 基于新vision修复 (login-page-v4)
      上下文包含：
        - 已锁定的v3代码（除颜色外的逻辑保持不变）
        - 新的颜色规范
        - 指示：只改颜色，不要动逻辑
```

---

## 十二、工具迁移参考

| 功能 | 迁移工具 | 说明 |
|------|---------|------|
| **Skill同步** | skillshare | 单向源多目标，60+工具支持 |
| **MCP配置同步** | cc-switch | 桌面端，5个工具统一管理 |
| **批量配置迁移** | ai-sync | 命令行，自动格式转换 |
| **Skill包安装** | agr | 一键安装到OpenCode |

---

*文档生成时间: 2026-05-06*  
*基于多轮对话整理*

**关联仓库文档**：[`../orchestration/agent-collab-protocol-v1.md`](../orchestration/agent-collab-protocol-v1.md)（应用层协议 v1）、[`../product/任务清单-v2.md`](../product/任务清单-v2.md)（实现任务与里程碑）、[`./spectrum-vixen-banshee.md`](./spectrum-vixen-banshee.md)（y2m 能力评估与映射建议）。
