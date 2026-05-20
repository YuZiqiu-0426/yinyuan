//! HTTP 路由与请求校验。

use axum::{
    extract::Json as RequestJson,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiOk<T> {
    code: &'static str,
    data: T,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiError {
    code: &'static str,
    message: &'static str,
    request_id: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiMessage<T> {
    code: &'static str,
    data: T,
    message: &'static str,
    request_id: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebLoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebMfaVerifyRequest {
    mfa_ticket: String,
    totp_code: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebSessionPayload {
    access_token: String,
    expires_in: u64,
    session_id: String,
    session_state: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MfaRequiredPayload {
    mfa_ticket: String,
    expires_in_seconds: u64,
}

fn ok<T: Serialize>(data: T) -> Response {
    Json(ApiOk { code: "OK", data }).into_response()
}

fn error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    (
        status,
        Json(ApiError {
            code,
            message,
            request_id: "req_stub",
        }),
    )
        .into_response()
}

fn session_payload(subject: &str, reason: &str) -> WebSessionPayload {
    let safe_subject = if subject.is_empty() {
        "anonymous"
    } else {
        subject
    };

    WebSessionPayload {
        access_token: format!("stub-access-{reason}-{safe_subject}"),
        expires_in: 900,
        session_id: format!("sess_stub_{reason}_{safe_subject}"),
        session_state: "active",
    }
}

async fn web_login(RequestJson(body): RequestJson<WebLoginRequest>) -> Response {
    if body.password == "wrong" {
        return error(
            StatusCode::UNAUTHORIZED,
            "AUTH_INVALID_CREDENTIALS",
            "invalid credentials",
        );
    }

    let username = body.username.trim().to_ascii_lowercase();
    if matches!(username.as_str(), "superadmin" | "groupadmin") {
        return Json(ApiMessage {
            code: "AUTH_MFA_REQUIRED",
            data: MfaRequiredPayload {
                mfa_ticket: format!("mfa_{username}_stub"),
                expires_in_seconds: 300,
            },
            message: "需要完成二次验证",
            request_id: "req_stub",
        })
        .into_response();
    }

    ok(session_payload(&username, "login"))
}

async fn web_mfa_verify(RequestJson(body): RequestJson<WebMfaVerifyRequest>) -> Response {
    if !body.mfa_ticket.starts_with("mfa_") {
        return error(
            StatusCode::UNAUTHORIZED,
            "AUTH_MFA_EXPIRED",
            "mfa ticket expired",
        );
    }

    if body.totp_code != "123456" {
        return error(
            StatusCode::UNAUTHORIZED,
            "AUTH_MFA_INVALID",
            "mfa code invalid",
        );
    }

    ok(session_payload("mfa", "mfa"))
}

async fn web_refresh(headers: HeaderMap) -> Response {
    if !headers.contains_key("x-csrf-token") {
        return error(
            StatusCode::FORBIDDEN,
            "AUTH_CSRF_INVALID",
            "csrf token is required",
        );
    }

    ok(session_payload("refresh", "refresh"))
}

fn api_v1_router() -> Router {
    Router::new()
        .route("/auth/web/login", post(web_login))
        .route("/auth/web/mfa/verify", post(web_mfa_verify))
        .route("/auth/web/refresh", post(web_refresh))
}

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/", get(root))
        .nest("/api/v1", api_v1_router())
}
