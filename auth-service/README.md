# auth-service（统一认证中心）

Rust + Axum 的**占位工程**，与 **`y2m-server`（消息中继）**、**`y2-manage`（管理端）** 解耦部署。详细模块划分见仓库 [`docs/auth/统一认证中心详细设计-v1.md`](../docs/auth/统一认证中心详细设计-v1.md) §4.1；HTTP 契约见 [`docs/auth/统一认证中心API定义-v1.md`](../docs/auth/统一认证中心API定义-v1.md)。

## 构建与运行

本 crate **不在** `y2m-rs` workspace 内，请在**本目录**执行：

```bash
cd auth-service
cargo build
cargo run
```

默认监听 **`127.0.0.1:8090`**（避免与 `y2m-server` 默认 **8080** 冲突）。监听地址：

```bash
# Linux / macOS
export AUTH_SERVICE_BIND=127.0.0.1:8090
cargo run
```

```powershell
# Windows PowerShell
$env:AUTH_SERVICE_BIND = "127.0.0.1:8090"
cargo run
```

## 探活

```bash
curl http://127.0.0.1:8090/health
```

根路径 **`GET /`** 返回纯文本服务名与版本，便于浏览器快速辨认。

## 阶段 A：最小 Web 认证 Stub

当前已提供与 [`docs/auth/统一认证中心API定义-v1.md`](../docs/auth/统一认证中心API定义-v1.md) 对齐的最小 HTTP stub，便于 `y2-manage` 关闭 Mock 后本地联调：

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/api/v1/auth/web/login` | 密码登录；`password == "wrong"` 返回 `AUTH_INVALID_CREDENTIALS` |
| `POST` | `/api/v1/auth/web/mfa/verify` | MFA 二次校验；固定验证码 `123456` |
| `POST` | `/api/v1/auth/web/refresh` | 刷新 access token；需携带 `X-CSRF-Token` |

Stub 规则：

- `username` 为 `superadmin` 或 `groupadmin` 时，登录返回 `AUTH_MFA_REQUIRED` 与 `mfaTicket`。
- 其他用户名只要密码不是 `wrong`，直接返回 `{ code: "OK", data: { accessToken, expiresIn, sessionId, sessionState } }`。
- 当前 token / session 均为可预测的 stub 字符串，不做数据库、Redis、JWT 签名或真实密码校验。
- CORS 允许 `http://localhost:4200` 与 `http://127.0.0.1:4200`，并允许 credentials，供 Angular dev server 联调。

`y2-manage` 本地联调时，可临时把 `frontend-monorepo/apps/y2-manage/src/environments/environment.development.ts` 调整为：

```ts
apiBaseUrl: 'http://127.0.0.1:8090',
useAuthMock: false,
devCsrfToken: 'dev-mock-csrf',
```

## PostgreSQL / Redis 要不要先装？

**当前阶段 A 不用。** 本工程尚未依赖 SQLx、Redis 客户端；本地只需 **Rust 工具链**即可编译运行。

接入持久化与会话缓存时（见设计文档 §3），再任选其一即可：

- **本机安装** PostgreSQL / Redis；或  
- **Docker Compose** 起官方镜像（与任务清单 **INF-01** 一类联调环境对齐，便于团队同版本）。

迁移目录与 DB 子清单见 **[`docs/product/任务清单-v2.md`](../docs/product/任务清单-v2.md)**（**模块二 · 数据库迁移拆解**）；表结构细节见 [`docs/auth/数据库设计与迁移方案-v1.md`](../docs/auth/数据库设计与迁移方案-v1.md)。（[`任务清单-v1.md`](../docs/product/任务清单-v1.md) 已废弃，勿作排期依据。）

## 后续

数据库、Redis、JWT、真实密码校验与持久化会话等按设计文档分阶段接入；当前 crate **不**包含 SQLx 或迁移脚本。
