//! HTTP 路由与请求校验（当前仅健康检查）。

use axum::{routing::get, Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct HealthBody {
    status: &'static str,
}

async fn health() -> Json<HealthBody> {
    Json(HealthBody { status: "ok" })
}

async fn root() -> &'static str {
    concat!("auth-service ", env!("CARGO_PKG_VERSION"))
}

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/", get(root))
}
