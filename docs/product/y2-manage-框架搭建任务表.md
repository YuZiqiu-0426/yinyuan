# y2-manage 框架搭建任务表（详细）

## 1. 文档目的

- 把 **`frontend-monorepo/apps/y2-manage`**（Angular 管理端）从「脚手架」推到 **M3：管理端可用**（[`任务清单-v2.md`](./任务清单-v2.md) 模块七 **MW-01～MW-08**）的**可执行拆解清单**写在一处，便于排期、分工与验收。
- **不替代** [`任务清单-v2.md`](./任务清单-v2.md) 中的 MW 编号与依赖列；本表在其上补充**子任务、验收口径、横向工程项**。
- **总进度与优先级**仍以 [`工作进度.md`](./工作进度.md) **§5.3** 为准；本表完成后可在 §5.3 增加「详见本文」链接（已互链）。

## 2. 仓库位置与工程约定（必读）

| 项 | 说明 |
|----|------|
| 目录 | `frontend-monorepo/apps/y2-manage` |
| 包管理 | 仅在 **`frontend-monorepo/`** 使用 **pnpm**（见根目录 [`agent.md`](../../agent.md)） |
| 样式 | **`frontend-monorepo/apps/*`** Web 应用**强制 Tailwind CSS**；本应用全局入口为 **`src/styles.css`** + **`.postcssrc.json`**（见 `agent.md` §5） |
| 业务 HTTP | **一律**使用 **`@y2/shared`** 的 **`Y2HttpClient`**，禁止与 Angular `HttpClient` 混用于同一业务域（见 `agent.md` §5） |
| 构建验证 | `cd frontend-monorepo && pnpm --filter y2-manage build` |

## 3. 当前基线（实现事实，随 PR 更新）

| 项 | 状态 | 备注 |
|----|------|------|
| Angular 应用 + `ng build` | 已有 | `@angular/build:application` |
| 全局 Tailwind v4 | 已有 | `src/styles.css` 中 `@import "tailwindcss"`；`.postcssrc.json`；CSS 变量主题基线 |
| 路由与壳 | 已有 | `/login`、`/`（`MainShell` + 子路由）、`**` → 404；`loadComponent` 懒加载；`authGuard` / `loginShellGuard` |
| 环境与 HTTP | 已有 | `environment.*` + `fileReplacements`；`provideCore()` 提供带拦截器的 `Y2HttpClient`（Bearer、`401→refresh` 一次后重试） |
| 登录与会话（MW-01/02 部分） | 已有 | `AuthApiService` + `useAuthMock`；登录表单与错误码中文；内存 `accessToken`；`refreshTokens` 走独立 axios 避免递归；**MFA 未做** |
| `@y2/shared` | 已有 | 管理端经 `Y2_HTTP_CLIENT`；`ng serve` 见 README paths + `prebundle.exclude` |
| `auth-service` | 未接 | 生产登录/刷新需真实后端与 HttpOnly refresh + CSRF；Admin API 仍依赖 AU-* |

## 4. 总览：阶段 ↔ 里程碑 ↔ MW

| 阶段 | 主题 | 对应 MW（任务清单-v2） | 目标里程碑 |
|------|------|------------------------|------------|
| Phase 0 | 工程与规范对齐 | — | 可持续集成、团队约定已满足 |
| Phase 1 | 应用壳 + 路由骨架 | —（为 MW 铺路） | 可导航、可守卫、无占位首页 |
| Phase 2 | 登录与会话 | MW-01、MW-02 | 可登录、可刷新、可登出 |
| Phase 3 | 用户管理 | MW-03、MW-04 | 列表 + 创建/禁用闭环 |
| Phase 4 | 角色与权限 | MW-05 | 权限矩阵可编辑并保存 |
| Phase 5 | 设备与 CLI 审核 | MW-06、MW-07 | 列表 + 审核动作 |
| Phase 6 | 审计 | MW-08 | 检索 + 导出 |
| Phase 7 | Agent 看板与任务树 | MW-09、MW-10 | **M7**，依赖编排侧进度 |

## 5. Phase 0 — 工程与规范对齐

**目标**：后续功能开发不因工具链或约定返工。

- [x] **目录与命名**：约定 feature 目录结构（如 `src/app/core`、`shared`、`features/*`），与 Angular 风格指南一致。
- [x] **环境配置**：新增 `environment.ts` / `environment.development.ts`（或 v17+ 等价方案），至少包含 **`apiBaseUrl`**、**`authIssuer`** 等占位；生产构建不包含敏感默认值。
- [ ] **路径别名**：若需 `@app/*` 等，在 `tsconfig` 与 `angular.json` 中一次性配好。
- [ ] **Lint / Format**：与 monorepo 统一（ESLint、Prettier 或 Angular 默认；**Tab Size 2** 见 [`meta/文档导航-v1.md`](../meta/文档导航-v1.md) §8）。当前在 `apps/y2-manage/README.md` 注明「待与 monorepo 统一」。
- [x] **HTTP 基类**：封装 `Y2HttpClient` 单例或工厂（如 `provideAppApi()`），统一 `baseURL`、错误映射、401 跳转策略的挂载点（可与 Phase 2 一起做，但须在 Phase 2 前定接口）。
- [x] **验收**：`pnpm --filter y2-manage build` / `test`（若有）无回归；README 或 `apps/y2-manage/README.md` 中写清启动命令与环境变量说明。

## 6. Phase 1 — 应用壳与路由骨架

**目标**：去掉默认欢迎页，形成**可扩展壳**，与登录/业务路由分离。

### 6.1 布局

- [x] **根布局组件**：顶栏（产品名、当前用户、登出入口占位）、侧栏导航（菜单项可先占位链接）、主内容区 `<router-outlet>`。
- [x] **响应式**：侧栏在窄屏可折叠（Tailwind `md:` 等）；键盘可达性（焦点顺序、`aria-*`）。
- [ ] **空状态与加载**：全局 `router-outlet` 外层统一加载指示占位（可与 Phase 2 共用设计 token）。

### 6.2 路由

- [x] **路由表**：至少划分 **`/login`**、**`/`（受保护）**、**`/**` 回退**；懒加载各 feature 模块（用户、角色、设备等可拆为 `routes.ts`）。
- [x] **默认重定向**：已登录访问 `/login` → 首页；未登录访问受保护路由 → `/login`。
- [x] **路由守卫**：`canActivate` / `canMatch` 占位：先以「内存中是否有 token」伪实现，待 Phase 2 接真实会话。
- [x] **404**：未知路径进入统一错误页。

### 6.3 样式与主题

- [x] **清除占位**：移除或替换 Angular 默认大段欢迎 HTML/CSS，避免与 Tailwind 混用产生双体系。
- [x] **设计基线**：字体、圆角、主色在 `styles.css` 或 Tailwind `@theme`（若采用 v4 主题扩展）中集中定义。

### 6.4 验收

- [ ] 手工：冷启动 `pnpm --filter y2-manage start`，导航与刷新后路由状态正确。
- [x] 构建：`ng build` 无 budget 违规（注意组件样式体积）。

## 7. Phase 2 — 登录与会话（MW-01 / MW-02）

**依赖**：[`任务清单-v2`](./任务清单-v2.md) **AU-04** 等；无后端时用 **Mock** 或契约对齐 [`docs/auth/统一认证中心API定义-v1.md`](../auth/统一认证中心API定义-v1.md)。

### 7.1 MW-01 登录页

- [x] 表单：用户名 + 密码；校验与可访问性（`label`、`aria-invalid`）。
- [ ] **MFA**：`super_admin` **强制** MFA 流程（与 [`docs/auth/统一认证与双Web端需求规格-v1.md`](../auth/统一认证与双Web端需求规格-v1.md) 一致）；步骤 UI（如 TOTP 二次页）。
- [x] 错误展示：接口错误码 → 用户可读中文（对齐 API 文档错误语义）。
- [x] 登录成功：写入 token（当前：**仅内存** accessToken + session 元数据；真实 Web 的 refresh 依赖 HttpOnly Cookie，见 README 与加密规范；**MFA 前为占位策略**）。

### 7.2 MW-02 会话管理

- [x] **Access Token 静默刷新**：`Y2HttpClient` 拦截器在 **401** 时触发一次 `refreshTokens`（独立 axios，避免与拦截器递归）；**未**做定时主动刷新。
- [x] **登出**：清内存 token / session 字段并重定向 `/login`；无服务端 revoke（待有 API）。
- [ ] **路由守卫**：与刷新逻辑协同，避免刷新竞态导致误跳登录（当前为单次 refresh + 单次重试；高并发竞态待有后端再压测）。
- [x] **401 统一处理**：业务请求 401 → 尝试刷新一次 → 仍失败则登出。

### 7.3 验收

- [ ] 有后端：走通登录 + MFA + 刷新 + 登出全流程。
- [x] 无后端：`useAuthMock` 下 Mock 与文档 JSON 形状一致，`ng serve` 可演示登录与错误（**不含 MFA 全流**）。

## 8. Phase 3 — 用户管理（MW-03 / MW-04）

**依赖**：**AU-12**（任务清单）。

### 8.1 MW-03 用户列表

- [ ] 表格：分页、排序（若 API 支持）、按用户名/邮箱 **搜索**、按状态 **筛选**。
- [ ] 空列表与加载失败状态。
- [ ] 权限：仅具相应 admin 权限的角色可见菜单与路由（与权限矩阵对齐）。

### 8.2 MW-04 用户创建 / 禁用

- [ ] 创建表单：字段与 API 对齐；前端校验 + 服务端错误展示。
- [ ] **高危操作二次确认**（禁用、删权等）：模态确认 + 明确后果文案（见需求规格）。
- [ ] 操作后刷新列表或乐观更新策略（二选一并文档化）。

## 9. Phase 4 — 角色与权限（MW-05）

**依赖**：**AU-12**。

- [ ] 角色列表与详情入口。
- [ ] **权限矩阵** UI：`PUT /admin/roles/{id}/permissions` 的批量编辑、保存、撤销未保存变更。
- [ ] 与 [`docs/auth/权限矩阵与默认角色模板-v1.md`](../auth/权限矩阵与默认角色模板-v1.md) 原子权限码展示一致（或可筛选分组）。

## 10. Phase 5 — 设备与 CLI 审核（MW-06 / MW-07）

**依赖**：**AU-09**、**AU-08**。

### 10.1 MW-06 设备管理

- [ ] 列表列：MAC、IP、OS 用户、状态等（以后端字段为准）。
- [ ] 封禁/解封（若 API 提供）：二次确认 + 审计提示。

### 10.2 MW-07 CLI 首登审核

- [ ] 待审核队列列表；详情抽屉/页（指纹信息展示）。
- [ ] 通过 / 拒绝动作 + 原因（若 API 需要）；与通知或轮询策略（可选）。

## 11. Phase 6 — 审计（MW-08）

**依赖**：**AU-13**。

- [ ] 检索条件：用户、IP、时间区间；分页。
- [ ] 导出 **CSV / JSON**（流式或分页导出，避免超大单次响应）。
- [ ] 大结果集时的 UX（限制条数提示、导出异步任务若后端支持）。

## 12. Phase 7 — Agent 看板与任务树（MW-09 / MW-10，M7）

**依赖**：编排 **OR-***、**OR-04**、**OR-01** 等；与 M3 可并行筹备，但**完整数据**依赖后端编排接口就绪。

- [ ] **MW-09**：在线 Agent 列表、能力矩阵、负载、任务状态卡片/表。
- [ ] **MW-10**：任务依赖图（可先用简化 DAG 组件）、迭代历史时间线、状态筛选。

## 13. 横向事项（贯穿多阶段）

建议在对应 Phase 内勾选，避免遗漏。

| 类别 | 内容 |
|------|------|
| 安全 | HTTPS 部署假设、Cookie `Secure/SameSite`、不在控制台打印 token |
| 无障碍 | 表单与表格基础 a11y；键盘可操作模态框 |
| 国际化 | v1 可仅中文；若引入 i18n，约定 key 前缀与懒加载 |
| 错误与空状态 | 全局错误边界或统一错误页；网络失败重试提示 |
| 性能 | 路由懒加载；大表虚拟滚动（按需） |
| 测试 | 核心守卫 + 登录服务单元测试；关键流 e2e（Playwright/Cypress 视团队选型后续加） |
| CI | `pnpm install` + `pnpm --filter y2-manage build` 纳入流水线 |

## 14. auth-service 依赖简表（与任务清单一致）

| MW | 主要依赖（任务清单-v2） |
|----|-------------------------|
| MW-01 | AU-04 等 |
| MW-02 | MW-01 |
| MW-03 | AU-12 |
| MW-04 | MW-03 |
| MW-05 | AU-12 |
| MW-06 | AU-09 |
| MW-07 | AU-08 |
| MW-08 | AU-13 |
| MW-09 | OR-04 |
| MW-10 | OR-01 |

后端未就绪时：**Mock 契约**不得长期与 OpenAPI/文档漂移；合并真实后端前做一次对齐评审。

## 15. 文档维护

- 完成某一 Phase 或 MW 后：在 [`工作进度.md`](./工作进度.md) **§5.3** 或变更记录中记一笔；**不必**把大段实现细节复制进《当前实现说明》（除非影响全仓事实基线）。
- 本表顶部「当前基线」表格应在关键合并后**随手更新**，避免与仓库脱节。

---

## 变更记录

| 日期 | 摘要 |
|------|------|
| 2026-05-10 | Phase 0/1 落地：环境切片、`provideCore`/`Y2HttpClient`、占位守卫、壳路由与 404；任务表 §3/§5/§6 同步 |
| 2026-05-10 | Phase 2 部分：登录表单、`AuthApiService`、Mock、`401→refresh` 拦截器、错误码中文；MFA 与有后端验收仍待 |
| 2026-05-12 | 初版：分 Phase 0～7 + 横向项 + AU 依赖简表；与任务清单 MW、工作进度 §5.3 对齐 |
