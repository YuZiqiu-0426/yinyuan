# y2-manage（隐元管理端）

Angular 20 应用，业务 HTTP 使用 `@y2/shared` 的 `Y2HttpClient`（`InjectionToken` `Y2_HTTP_CLIENT`，由 `provideCore()` 注册）。

## 开发与构建

在 monorepo 根目录执行：

```bash
cd frontend-monorepo
pnpm install
pnpm --filter y2-manage start
```

或使用根脚本：

```bash
pnpm run dev:manage
```

生产构建（CI 与验收）：

```bash
pnpm --filter y2-manage build
```

## 环境变量

环境切片位于 `src/environments/`：

| 文件 | 用途 |
|------|------|
| `environment.ts` | 默认再导出开发切片；**构建时**由 `angular.json` 的 `fileReplacements` 替换 |
| `environment.development.ts` | 本地 / `development` 构建 |
| `environment.production.ts` | `production` 构建 |

| 字段 | 说明 |
|------|------|
| **`apiBaseUrl`** | `axios` / `Y2HttpClient` 的 `baseURL`；生产可为 `''`（同源相对路径）。 |
| **`authIssuer`** | 统一认证占位说明。 |
| **`devBypassAuth`** | `true` 时跳过受保护路由的 token 检查（仅调试用，**勿用于生产**）。 |
| **`useAuthMock`** | `true` 时 `AuthApiService` 不调真实网络，返回与 `docs/auth/统一认证中心API定义-v1.md` 一致的 JSON 形状；**生产必须为 `false`**。 |
| **`devCsrfToken`** | 非 Mock 时随 `POST /api/v1/auth/web/refresh` 发送 `X-CSRF-Token` 的占位；真实环境应由页面注入服务端下发的 CSRF。 |
| **`refreshSkewSeconds`** | 临期主动 refresh：在 access 过期前多少秒触发静默 `refreshTokens`（与 401 拦截器共用 **单飞** `refreshTokens`）。开发默认 `10`，生产默认 `60`。 |

## 认证与 token 策略（当前实现）

- **Access Token**：登录成功后保存在内存（`AuthSessionService`），请求经拦截器附加 `Authorization: Bearer …`。
- **Refresh**：`POST /api/v1/auth/web/mfa/verify` 与 `POST /api/v1/auth/web/login` **不**附加 Bearer；`POST /api/v1/auth/web/refresh` 使用**独立 axios 实例**（不经 `Y2HttpClient` 拦截器），避免与 `401→refresh` 递归；文档要求 Cookie + CSRF，Mock 阶段用 `useAuthMock` 模拟响应。
- **401**：`Y2HttpClient` 拦截器对业务请求 **401** 时调用 `AuthApiService.refreshTokens()`（内部 **Promise 单飞**，多请求并发只发起一次 refresh），成功后重试原请求一次；仍失败则 `logout()`。MFA 校验请求若返回 401 **不**走 refresh 链，交由登录页展示错误。
- **临期 refresh**：根组件构造时注入 `SessionRenewScheduler`，根据 `accessExpiresAtMs` 与 `refreshSkewSeconds` 调度一次静默 refresh，与 401 路径共用同一单飞实现。
- **MFA（两步）**：契约见 `docs/auth/统一认证中心API定义-v1.md` §3.1.1。`mfaTicket` 仅存内存（`AuthSessionService`），登录成功或返回上一步时清除；第二步 UI 提示勿整页刷新。

## 与 `@y2/shared`（workspace）及 `ng serve`

开发服务器会对依赖做 Vite 预构建；workspace 包 `@y2/shared` 在预构建阶段用 esbuild 解析相对路径时易失败。当前做法：

- `angular.json` → `serve.options.prebundle.exclude` 包含 `@y2/shared`；
- `tsconfig.app.json` / `tsconfig.spec.json` 中通过 `paths` 将 `@y2/shared` 指到 `../../packages/shared/src/index.ts`，由应用编译器一并打包。

## Lint

本包尚未单独配置 ESLint；与 monorepo 全仓 Lint/Format 统一的工作待后续迭代。

## 路由概要

| 路径 | 说明 |
|------|------|
| `/login` | 用户名/密码登录，必要时第二步 TOTP；`?returnUrl=` 需为站内路径（以 `/` 开头且非 `//`） |
| `/`、`/dashboard` | `MainShell` + 总览占位（`canActivate`：`authGuard`） |
| 其他 | `404` |

已登录访问 `/login` 会重定向至 `/dashboard`（`loginShellGuard`）。

## Mock 手测提示

开发环境默认 `useAuthMock: true`：

- 密码 **`wrong`** → 模拟 `AUTH_INVALID_CREDENTIALS`。
- 用户名 **`superadmin`** 或 **`groupadmin`**（不区分大小写）且密码非 `wrong` → 进入 **MFA 第二步**；验证码填 **`123456`** 登录成功；其他 6 位数字 → `AUTH_MFA_INVALID`；篡改或过期票据（非 `mfa_` 前缀）→ `AUTH_MFA_EXPIRED`（Mock 行为）。
- 其余账号：任意非空密码直接登录成功（单步）。
