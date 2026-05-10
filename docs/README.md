# `docs/` 文档索引

本目录按主题分子目录，便于按角色查找；文件名未改，仅调整路径。

## 子目录职责

| 目录 | 内容 |
|------|------|
| [`meta/`](meta/) | 工程开发标准、文档导航与阅读顺序 |
| [`product/`](product/) | 当前实现说明、工作进度、任务清单（v1 / v2）、[`y2-manage-框架搭建任务表.md`](product/y2-manage-框架搭建任务表.md) |
| [`auth/`](auth/) | 统一认证中心需求、详细设计、API、权限、会话状态机、错误码、安全演练、配置与密钥、传输加密、数据库设计 |
| [`ops/`](ops/) | 编译部署、使用手册、上线与回滚 Runbook |
| [`orchestration/`](orchestration/) | 多 Agent 应用层协作协议（`agent-collab`） |
| [`requirements/`](requirements/) | 原始需求（v1） |
| [`strategy/`](strategy/) | 多 Agent 前端协作方案、能力与协议对照说明 |

随 **`y2m`** / **`y2m-server`** 的开发者文档仍在 [`../y2m-rs/docs/quickstart.md`](../y2m-rs/docs/quickstart.md)，不放在本索引树下。

**`auth-service/`**（统一认证中心，与 `y2m-rs` 分立）的本地运行与占位说明见 **[`../auth-service/README.md`](../auth-service/README.md)**；设计/API 仍以本目录 [`auth/`](auth/) 为准。

## 必读入口

1. **[`product/当前实现说明.md`](product/当前实现说明.md)** — 已实现能力的事实基线（接任务前优先读）  
2. **[`meta/文档导航-v1.md`](meta/文档导航-v1.md)** — 全量文档清单与推荐阅读顺序  
3. **[`../agent.md`](../agent.md)** — 仓库根目录：工程约束、构建/测试入口与导航  

工程标准与多文档冲突时的解释顺序见 **[`meta/工程开发标准-v1.md`](meta/工程开发标准-v1.md)** 第 3 节。
