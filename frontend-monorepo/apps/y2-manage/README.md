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

## 认证与 token 策略（当前实现）

- **Access Token**：登录成功后保存在内存（`AuthSessionService`），请求经拦截器附加 `Authorization: Bearer …`。
- **Refresh**：`POST /api/v1/auth/web/refresh` 使用**独立 axios 实例**（不经 `Y2HttpClient` 拦截器），避免与 `401→refresh` 递归；文档要求 Cookie + CSRF，Mock 阶段用 `useAuthMock` 模拟响应。
- **401**：`Y2HttpClient` 拦截器对业务请求 **401** 时尝试 **一次** refresh，重试原请求；仍失败则 `logout()`。
- **MFA**：尚未实现（见任务表 Phase 2 §7.1）。

## 与 `@y2/shared`（workspace）及 `ng serve`

开发服务器会对依赖做 Vite 预构建；workspace 包 `@y2/shared` 在预构建阶段用 esbuild 解析相对路径时易失败。当前做法：

- `angular.json` → `serve.options.prebundle.exclude` 包含 `@y2/shared`；
- `tsconfig.app.json` / `tsconfig.spec.json` 中通过 `paths` 将 `@y2/shared` 指到 `../../packages/shared/src/index.ts`，由应用编译器一并打包。

## Lint

本包尚未单独配置 ESLint；与 monorepo 全仓 Lint/Format 统一的工作待后续迭代。

## 路由概要

| 路径 | 说明 |
|------|------|
| `/login` | 用户名/密码登录；`?returnUrl=` 需为站内路径（以 `/` 开头且非 `//`） |
| `/`、`/dashboard` | `MainShell` + 总览占位（`canActivate`：`authGuard`） |
| 其他 | `404` |

已登录访问 `/login` 会重定向至 `/dashboard`（`loginShellGuard`）。

## Mock 手测提示

开发环境默认 `useAuthMock: true`：密码填 **`wrong`** 可模拟失败；其他任意非空密码视为登录成功。
