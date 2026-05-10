# 统一认证中心 API 定义 v1

## 1. 约定

- Base URL: `/api/v1`
- 响应格式：
  - 成功：`{ "code": "OK", "data": ... }`
  - 失败：`{ "code": "<ERROR_CODE>", "message": "...", "requestId": "..." }`
- 鉴权头：`Authorization: Bearer <access_token>`
- Web 刷新接口必须校验 CSRF Token 与 `Origin/Referer`。

## 2. 会话状态

- `active`：可收可发（按权限判定）
- `suspended_readonly`：仅接收，不可发送
- `revoked`：完全失效

## 3. Auth API

## 3.1 Web 登录

`POST /auth/web/login`

请求：

```json
{
  "username": "alice",
  "password": "******"
}
```

响应：

```json
{
  "code": "OK",
  "data": {
    "accessToken": "<jwt>",
    "expiresIn": 900,
    "sessionId": "sess_xxx",
    "sessionState": "active"
  }
}
```

说明：
- Refresh Token 通过 HttpOnly Cookie 下发，仅 Web 使用。

### 3.1.1 Web 登录 — MFA 二次校验

当用户密码正确且账户策略要求 MFA（如 `super_admin` / 部分 `group_admin`）时，**不**在第一步返回 `accessToken`，而是返回需二次校验的票据：

响应（HTTP 200，`code` 非 `OK`）：

```json
{
  "code": "AUTH_MFA_REQUIRED",
  "data": {
    "mfaTicket": "mfa_xxx",
    "expiresInSeconds": 300
  },
  "message": "需要完成二次验证",
  "requestId": "req_xxx"
}
```

`POST /auth/web/mfa/verify`

请求：

```json
{
  "mfaTicket": "mfa_xxx",
  "totpCode": "123456"
}
```

响应：与密码登录成功相同（`code: "OK"`，`data` 含 `accessToken`、`expiresIn`、`sessionId`、`sessionState`）。

说明：
- `mfaTicket` 为短时一次性票据，过期后须重新从 `/auth/web/login` 获取。
- 客户端应在完成 MFA 前避免整页刷新导致票据丢失（若仅内存持有）。

## 3.2 Web 刷新

`POST /auth/web/refresh`

请求：无 body（从 Cookie 读取 refresh token），Header 必须携带 `X-CSRF-Token`

响应：

```json
{
  "code": "OK",
  "data": {
    "accessToken": "<jwt>",
    "expiresIn": 900,
    "sessionId": "sess_xxx",
    "sessionState": "active"
  }
}
```

失败语义：
- Refresh 失效时将当前会话置为 `suspended_readonly`。
- 返回 `AUTH_REFRESH_EXPIRED`。
- 触发重放/高风险事件时会话直接置为 `revoked` 并断开连接。

## 3.3 CLI 登录（首次需审核）

`POST /auth/cli/login`

请求：

```json
{
  "username": "bob",
  "groupName": "default",
  "mac": "00-11-22-33-44-55",
  "osLoginUser": "bob-local",
  "deviceNonce": "nonce_xxx",
  "deviceSignature": "base64(signature)"
}
```

可能响应 1（待审核）：

```json
{
  "code": "AUTH_CLI_REVIEW_PENDING",
  "data": {
    "reviewId": "rvw_xxx",
    "status": "pending_review"
  }
}
```

可能响应 2（已通过）：

```json
{
  "code": "OK",
  "data": {
    "accessToken": "<jwt>",
    "expiresIn": 900,
    "sessionId": "sess_xxx",
    "sessionState": "active"
  }
}
```

规则：
- 首次登录必须由 `group_admin` 或 `super_admin` 审核通过。
- 每次会话都要重新提交并校验 `mac + ip + osLoginUser`。
- `ip` 以服务端观测到的源 IP 为准，不信任客户端上报值。
- `mac + osLoginUser` 必须通过设备私钥签名校验，防止伪造。

## 3.4 获取当前身份

`GET /auth/me`

响应：

```json
{
  "code": "OK",
  "data": {
    "userId": "usr_xxx",
    "username": "alice",
    "roles": ["user"],
    "groups": ["default"],
    "permissions": ["text.send", "text.recv"],
    "sessionId": "sess_xxx",
    "sessionState": "active"
  }
}
```

## 4. Admin API

## 4.1 审核 CLI 首次登录

`POST /admin/cli-reviews/{reviewId}/approve`

请求：

```json
{
  "comment": "设备信息确认通过"
}
```

`POST /admin/cli-reviews/{reviewId}/reject`

请求：

```json
{
  "reason": "设备与组不匹配"
}
```

权限规则：
- `super_admin`：可审核所有组
- `group_admin`：仅可审核所属组

## 4.2 用户创建

`POST /admin/users`

请求：

```json
{
  "username": "new-user",
  "initialPassword": "******",
  "groupName": "default",
  "roleCode": "user"
}
```

权限规则：
- `super_admin` 可创建任意组用户
- `group_admin` 仅可创建所属组用户

## 5. Chat 鉴权约束（服务端）

- `POST /chat/text/send` 需要 `text.send` 且 `sessionState=active`
- 广播/推送给客户端前检查：
  - 会话状态为 `active` 或 `suspended_readonly`
  - 且具备 `text.recv`
- `suspended_readonly` 禁止一切发送与管理操作
- `revoked` 立即拒绝并断开连接
- 高风险事件（token 重放、管理员强制下线、异地异常）必须直接转 `revoked`，不可仅降级为只读

## 6. 错误码

- `AUTH_INVALID_CREDENTIALS`
- `AUTH_TOKEN_EXPIRED`
- `AUTH_REFRESH_EXPIRED`
- `AUTH_IP_MISMATCH`
- `AUTH_DEVICE_UNTRUSTED`
- `AUTH_CLI_REVIEW_PENDING`
- `AUTH_CLI_REVIEW_REJECTED`
- `AUTH_PERMISSION_DENIED`
- `AUTH_SESSION_READONLY`
- `AUTH_SESSION_REVOKED`
- `AUTH_GROUP_SCOPE_DENIED`
- `AUTH_CSRF_INVALID`
- `AUTH_RISK_REVOKED`
- `AUTH_MFA_REQUIRED`（登录第一步：需继续调用 `/auth/web/mfa/verify`）
- `AUTH_MFA_INVALID`
- `AUTH_MFA_EXPIRED`

## 7. 说明

- `json/command/file` 在 v1 不开放前端 UI。
- CLI 不接收 Refresh 失效通知，安全控制由服务端实时鉴权保证。
