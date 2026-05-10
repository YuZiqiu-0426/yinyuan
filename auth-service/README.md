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

## PostgreSQL / Redis 要不要先装？

**当前架子不用。** 本工程尚未依赖 SQLx、Redis 客户端；本地只需 **Rust 工具链**即可编译运行。

接入持久化与会话缓存时（见设计文档 §3），再任选其一即可：

- **本机安装** PostgreSQL / Redis；或  
- **Docker Compose** 起官方镜像（与任务清单 **INF-01** 一类联调环境对齐，便于团队同版本）。

迁移目录等约定可参考 [`docs/product/任务清单-v1.md`](../docs/product/任务清单-v1.md) 模块二说明（实现时再落地）。

## 后续

数据库、Redis、JWT、真实 **`/api/v1/auth/web/*`** 路由等按设计文档分阶段接入；当前 crate **不**包含 SQLx 或迁移脚本。
