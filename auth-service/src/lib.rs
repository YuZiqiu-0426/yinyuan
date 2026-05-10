//! 统一认证中心 HTTP 服务（架子）：路由入口见 [`api`]，其余模块与设计文档 §4.1 对齐占位。

pub mod api;
pub mod application;
pub mod audit;
pub mod domain;
pub mod infra;

use std::net::SocketAddr;

use anyhow::Context;
use axum::Router;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// 解析监听地址：`AUTH_SERVICE_BIND`，默认 `127.0.0.1:8090`。
pub fn parse_bind_addr() -> anyhow::Result<SocketAddr> {
    let s = std::env::var("AUTH_SERVICE_BIND").unwrap_or_else(|_| "127.0.0.1:8090".to_string());
    s.parse().with_context(|| format!("invalid AUTH_SERVICE_BIND: {s}"))
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// 构建应用路由（健康检查等）。
pub fn app() -> Router {
    Router::new().merge(api::router())
}

/// 初始化日志、绑定地址并启动 Axum。
pub async fn run() -> anyhow::Result<()> {
    init_tracing();
    let addr = parse_bind_addr()?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "auth-service listening");
    axum::serve(listener, app()).await?;
    Ok(())
}
