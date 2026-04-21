use std::net::SocketAddr;

use tracing_appender::non_blocking::WorkerGuard;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _log_guard = init_logging();

    let addr = std::env::var("Y2M_SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
        .parse::<SocketAddr>()?;

    y2m_server::serve(addr).await
}

fn init_logging() -> WorkerGuard {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let log_path = std::env::var("Y2M_SERVER_LOG").unwrap_or_else(|_| "y2m-server.log".to_string());
    let file = std::fs::OpenOptions::new()
        .create(true).append(true).open(&log_path)
        .unwrap_or_else(|e| panic!("无法打开日志文件 {log_path}: {e}"));
    let (file_writer, guard) = tracing_appender::non_blocking(file);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        // stderr: JSON 格式，方便运维工具解析
        .with(fmt::layer().with_writer(std::io::stderr).json())
        // 文件: 人类可读格式，无 ANSI 色码
        .with(fmt::layer().with_writer(file_writer).with_ansi(false))
        .init();

    guard
}
