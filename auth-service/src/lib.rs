//! 统一认证中心 HTTP 服务（架子）：路由入口见 [`api`]，其余模块与设计文档 §4.1 对齐占位。

pub mod api;
pub mod application;
pub mod audit;
pub mod domain;
pub mod infra;

use std::net::SocketAddr;

use anyhow::Context;
use axum::{
    http::{header, HeaderName, HeaderValue, Method},
    Router,
};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// 解析监听地址：`AUTH_SERVICE_BIND`，默认 `127.0.0.1:8090`。
pub fn parse_bind_addr() -> anyhow::Result<SocketAddr> {
    let s = std::env::var("AUTH_SERVICE_BIND").unwrap_or_else(|_| "127.0.0.1:8090".to_string());
    s.parse()
        .with_context(|| format!("invalid AUTH_SERVICE_BIND: {s}"))
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn dev_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:4200"),
            HeaderValue::from_static("http://127.0.0.1:4200"),
        ])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            HeaderName::from_static("x-csrf-token"),
        ])
        .allow_credentials(true)
}

/// 构建应用路由。
pub fn app() -> Router {
    Router::new().merge(api::router()).layer(dev_cors_layer())
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
