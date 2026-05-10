# auth-service（统一认证中心）

Rust + Axum 的**占位工程**，与 **`y2m-server`（消息中继）**、**`y2-manage`（管理端）** 解耦部署。详细模块划分见仓库 [`docs/auth/统一认证中心详细设计-v1.md`](../docs/auth/统一认证中心详细设计-v1.md) §4.1；HTTP 契约见 [`docs/auth/统一认证中心API定义-v1.md`](../docs/auth/统一认证中心API定义-v1.md)。

## 构建与运行

```bash
cd auth-service
cargo build
cargo run
```

默认监听 **`127.0.0.1:8090`**（避免与 `y2m-server` 默认 **8080** 冲突）。可通过环境变量覆盖：

```bash
set AUTH_SERVICE_BIND=127.0.0.1:8090
cargo run
```

## 探活

```bash
curl http://127.0.0.1:8090/health
```

根路径 **`GET /`** 返回纯文本服务名与版本，便于浏览器快速辨认。

## 后续

数据库（PostgreSQL）、Redis、JWT、真实 `/auth/web/*` 路由等按设计文档分阶段接入；当前 crate **不**包含 SQLx 或迁移。
