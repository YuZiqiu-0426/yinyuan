# 统一认证与双 Web 端需求规格 v1

## 1. 文档目标

本规格用于冻结第一阶段需求，作为后续接口设计、数据库建模和前后端开发的统一基线。

第一阶段仅实现文本聊天能力，同时完成统一认证中心与角色权限体系落地。

## 2. 范围与非范围

### 2.1 范围（In Scope）

- 统一认证中心（Auth Center）
- 普通用户 Web 客户端（Next.js）
- 管理端 Web 客户端（Angular）
- CLI 与 Web 的统一鉴权接入
- 三类角色与权限控制（super_admin / group_admin / user）
- 聊天能力：仅文本（text）
- 权限模型包含 `text/json/command/file` 的收发控制

### 2.2 非范围（Out of Scope）

- 语音/视频聊天
- 富文本与消息撤回
- 端到端加密完整实现（本期只定义接口与扩展点）
- 多租户隔离（先按单租户组织模型）

## 3. 角色与权限模型

## 3.1 系统角色

- `super_admin`：全局管理权限，可跨组管理用户、角色、策略和审计。
- `group_admin`：仅限所属组管理用户与权限，不可做全局系统配置，也不可创建跨组用户。
- `user`：普通用户，仅能访问被授权的通信能力。

### 3.2 权限原子项

每类能力拆分为发送与接收两个原子权限：

- `text.send` / `text.recv`
- `json.send` / `json.recv`
- `command.send` / `command.recv`
- `file.send` / `file.recv`

### 3.3 权限生效规则

- 连接建立时校验基础身份与组归属。
- 业务动作执行前逐项校验原子权限。
- 任一鉴权失败返回明确错误码并记入审计日志。

## 4. 统一认证中心（Auth Center）

### 4.1 职责

- 账号认证（Web）
- 设备认证（CLI）
- Token 签发、刷新、吊销
- 权限快照下发（角色 + 原子权限）
- 会话管理与审计追踪

### 4.2 认证因子

#### CLI

- 因子：`MAC 地址 + IP 地址 + 当前设备登录用户`
- 首次接入必须提交 `username + groupName`，用于账号与组归属校验
- CLI 首次登录必须由 `group_admin` 或 `super_admin` 审核通过后，方可发放设备会话凭证
- 每次会话都必须重新校验 `MAC + IP + 当前设备登录用户`
- CLI 因子必须通过设备签名证明，服务端不信任客户端上报 IP（以连接源 IP 为准）
- 通过后签发设备会话凭证

#### Web

- 因子：`JWT + IP`
- 登录后发放 Access Token 与 Refresh Token
- 请求时校验 Token 与来源 IP

### 4.3 Token 策略（建议）

- Access Token：15 分钟有效
- Refresh Token：仅用于 Web，7 天有效（可配置）
- 每次刷新轮换 Refresh Token（旧 token 立即失效）
- Refresh Token 失效后，会话降级为只读挂起（可接收，不可操作）
- 发生高风险事件（重放、管理员强制下线、异地异常）时会话直接 `revoked`
- 管理员可按用户、设备、角色维度强制吊销

## 5. 客户端边界

### 5.1 普通用户 Web（Next.js）

- 登录、登出、会话恢复
- 文本聊天界面（单聊/组聊）
- 消息列表与发送输入框
- 连接状态与基本错误提示
- 个人会话安全信息（最近登录、会话 IP）

### 5.2 管理端 Web（Angular）

- 用户创建、启用、禁用
- 用户绑定组、角色分配
- 权限模板配置与覆盖
- 设备记录查看与封禁
- 审计日志查看（筛选、导出）

## 6. 聊天与消息约束（v1）

- 仅上线 `text` 消息完整链路。
- `json/command/file` 权限先纳入认证与授权体系，但在 v1 明确不开放 UI 入口。
- 服务端消息路由前必须进行权限校验。
- 服务端在每次操作和每次广播分发前都必须校验会话状态（active/suspended_readonly/revoked）。
- 对权限不足、会话过期、IP 不匹配提供可观测错误码。

## 7. 数据模型（逻辑层）

最小实体集如下：

- `users`：用户主体
- `groups`：群组
- `roles`：角色定义（含系统角色）
- `permissions`：原子权限定义
- `role_permissions`：角色-权限映射
- `user_group_roles`：用户在组内的角色映射
- `devices`：CLI 设备指纹与状态
- `sessions`：Web/CLI 会话与 token 元数据
- `audit_logs`：审计日志（不可篡改、只追加）

## 8. 鉴权流程（概要）

### 8.1 Web 登录

1. 用户名密码登录认证中心  
2. 认证通过后签发 Access + Refresh  
3. 网关/服务端校验 JWT 与 IP  
4. 返回权限快照并建立会话

### 8.2 CLI 登录

1. CLI 上报 `MAC + IP + 当前登录用户`  
2. 认证中心校验设备状态与绑定关系  
3. 通过后签发 CLI 会话 token  
4. 服务端连接与发消息时按权限执行校验

## 9. 审计与安全要求

- 所有登录、鉴权失败、权限拒绝必须记录审计日志。
- 用户创建、角色变更、权限变更必须记录操作人和前后差异。
- 管理端高危操作需二次确认（例如禁用用户、权限提升）。
- 管理端在 v1 强制 MFA（`super_admin` 必选）。
- 审计日志支持按用户、IP、时间区间检索。
- IP 校验增强为 CIDR 白名单策略，支持单 IP 和网段规则。
- Web 刷新接口强制 CSRF 防护（SameSite + CSRF Token + Origin/Referer 校验）。

## 10. 错误码建议

- `AUTH_INVALID_CREDENTIALS`：账号或凭据错误
- `AUTH_TOKEN_EXPIRED`：访问 token 过期
- `AUTH_IP_MISMATCH`：token 与来源 IP 不一致
- `AUTH_DEVICE_UNTRUSTED`：CLI 设备不可信
- `AUTH_PERMISSION_DENIED`：无对应原子权限
- `AUTH_ACCOUNT_DISABLED`：账号被禁用

## 11. 里程碑与验收

### M1：认证中心最小可用

- Web 登录、刷新、登出完成
- CLI 因子校验链路可用
- 基础审计日志可查询

### M2：用户端 Web（Next.js）

- 完成文本聊天收发
- 完成会话鉴权与异常处理

### M3：管理端 Web（Angular）

- 用户创建与角色分配可用
- 权限模板配置可用
- 审计日志页面可用

### 验收标准（v1）

- 三类角色权限生效且可验证。
- Web 和 CLI 均接入统一认证中心。
- 文本聊天在权限允许时可稳定收发。
- 关键安全事件可追踪审计。

## 12. 待确认项

- 当前无（后续新增需求时再补充）。
