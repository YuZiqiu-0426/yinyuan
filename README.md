# 隐元（YinYuan）

> 基于 WebSocket 的异构 AI Agent 协作编排基础设施。

隐元提供安全的消息总线、统一认证与权限管控，支持多 Agent（Claude、Kimi、Cursor、OpenCode 等）通过 CLI 长连接进行任务分发、代码协作、文件传输与命令执行。

用户交互为 **CLI-only**（`y2m` 二进制）；管理端为 Web 应用（`y2-manage`，Angular）。

---

## 仓库结构

```text
YinYuan/
├── y2m-rs/                 ← Rust 主工作区（消息总线 + CLI）
│   ├── crates/server       ← y2m-server（WebSocket 中继）
│   ├── crates/client-core  ← y2m-client-core（连接、协议、插件分发）
│   ├── crates/common       ← y2m-common（协议模型、公共枚举）
│   ├── src/                ← y2m CLI（init / run / send / chat）
│   └── tests/              ← 进程内 e2e + CLI 进程 e2e
├── frontend-monorepo/      ← 前端 monorepo（仅管理端 y2-manage）
├── docs/                   ← 设计、规范、进度、实现说明
├── agent.md                ← 工程约定 + 仓库导航 + 构建/测试 + 架构（合并原 CONVENTIONS / CLAUDE）
├── CONVENTIONS.md          ← 重定向至 agent.md（兼容旧链接）
├── CLAUDE.md               ← 重定向至 agent.md（兼容旧链接）
└── README.md               ← 本文件（项目总入口）
```

---

## 快速开始

```bash
# 1. 构建 Rust 工作区
cd y2m-rs
cargo build --release

# 2. 启动服务端
./target/release/y2m-server
# 默认监听 127.0.0.1:8080，可用 Y2M_SERVER_ADDR 覆盖

# 3. 初始化客户端配置
./target/release/y2m init --config alice.json \
  --server-url ws://127.0.0.1:8080/ws \
  --group default --client alice

# 4. 进入交互式会话
./target/release/y2m chat --config alice.json
```

完整 walkthrough 见 [`y2m-rs/docs/quickstart.md`](y2m-rs/docs/quickstart.md)。

---

## 文档体系

### 开发入口（必读）

| 文档 | 用途 | 优先级 |
|------|------|--------|
| [`docs/当前实现说明.md`](docs/当前实现说明.md) | **实现事实基线**——代码现在做了什么、还没做什么 | 🔴 最高 |
| [`docs/工作进度.md`](docs/工作进度.md) | 任务状态总表（P0/P1/P2） | 🔴 最高 |
| [`agent.md`](agent.md) | 工程约定、Git/tmux/pnpm、仓库布局、`cargo`/测试入口、架构与安全导航 | 🟡 高 |

> **冲突规则**：若文档之间描述不一致，以 `当前实现说明.md` > `工作进度.md` > `agent.md` > 其他设计/API 文档为准。

### 项目方向

| 文档 | 说明 |
|------|------|
| [`多Agent前端协作开发方案.md`](多Agent前端协作开发方案.md) | 多 Agent 协作架构：分层流水线、Socket 消息协议、任务状态机、共享层机制 |
| [`docs/任务清单-v2.md`](docs/任务清单-v2.md) | 多 Agent 与编排路线：传输层、auth、agent-collab、编排器、管理端任务拆分与里程碑 |
| [`docs/agent-collab-protocol-v1.md`](docs/agent-collab-protocol-v1.md) | `agent-collab-v1` 应用层协议：y2m `Json` 承载、MessageType、状态对照与示例消息 |
| [`spectrum-vixen-banshee.md`](spectrum-vixen-banshee.md) | y2m-rs 胜任评估：传输层完全匹配，应用层需适配 `agent-collab` JSON schema |

### 认证中心与安全（子导航）

认证中心相关文档有独立导航：[`docs/文档导航-v1.md`](docs/文档导航-v1.md)。核心文档：

| 文档 | 说明 |
|------|------|
| [`docs/统一认证中心详细设计-v1.md`](docs/统一认证中心详细设计-v1.md) | auth-service 架构、JWT 会话、CLI 设备因子校验 |
| [`docs/统一认证中心API定义-v1.md`](docs/统一认证中心API定义-v1.md) | Auth / Admin / Chat 鉴权 API 与错误码 |
| [`docs/加密验证方案.md`](docs/加密验证方案.md) | 接入方式、密钥体系、TLS/mTLS、设备信任、审计告警 |
| [`docs/权限矩阵与默认角色模板-v1.md`](docs/权限矩阵与默认角色模板-v1.md) | RBAC 角色（super_admin / group_admin / user）与原子权限 |
| [`docs/配置与密钥管理规范-v1.md`](docs/配置与密钥管理规范-v1.md) | 环境变量、JWT 密钥轮换、安全基线 |
| [`docs/统一认证中心攻击路径演练与验收清单-v1.md`](docs/统一认证中心攻击路径演练与验收清单-v1.md) | 10 条攻击路径演练（暴力破解、Token 重放、CSRF、设备伪造等） |

### 运维与发布

| 文档 | 说明 |
|------|------|
| [`编译部署文档.md`](编译部署文档.md) | 构建与部署笔记 |
| [`docs/上线与回滚Runbook-v1.md`](docs/上线与回滚Runbook-v1.md) | 发布执行手册 |
| [`使用手册.md`](使用手册.md) | 终端用户操作指南 |

---

## 里程碑

| 阶段 | 目标 | 状态 |
|------|------|------|
| **M1** | 认证中心最小可用：Web/CLI 登录、token 刷新、会话吊销、CLI 设备审核流 | ⬜ |
| **M2** | CLI 客户端鉴权闭环：`y2m` 完成设备因子登录，`text` 收发权限校验生效 | ⬜ |
| **M3** | 管理端可用（`y2-manage`）：用户/角色/权限/设备审核页面、审计检索与导出 | ⬜ |
| **M4** | 多 Agent 编排层：[`docs/agent-collab-protocol-v1.md`](docs/agent-collab-protocol-v1.md) 协议、`y2m agent` 包装器、编排器、共享层锁定（任务见 [`docs/任务清单-v2.md`](docs/任务清单-v2.md)） | ⬜ |

---

## 技术栈

| 层级 | 技术 |
|------|------|
| 消息总线 | Rust + axum + tokio + tokio-tungstenite |
| CLI 客户端 | Rust + clap + reedline + tracing-appender |
| 管理端 Web | Angular + pnpm |
| 认证中心（planned）| Rust + axum + SQLx + PostgreSQL + Redis |
| 协议 | WebSocket + JSON（控制面）+ 二进制分片（文件传输）|
| 安全 | TLS 1.3 / mTLS + JWT（RS256）+ Argon2id + 设备指纹 |
