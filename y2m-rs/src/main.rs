mod cli;
mod cmd_chat;
mod cmd_init;
mod cmd_run;
mod cmd_send;
mod file_flow;
mod file_store;
mod line_editor;
mod plugin;
mod printer;
mod state;
mod types;
mod util;

use std::{
    io::{self, BufRead},
    sync::Arc,
    thread,
};

use tokio::sync::mpsc;
use y2m_client_core::{ClientConfig, ClientCore};

use crate::{
    cli::{Cli, Commands},
    plugin::ConsolePlugin,
    state::ConsoleState,
};

use clap::Parser;
use tracing_appender::non_blocking::WorkerGuard;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _log_guard = init_logging();
    let cli = Cli::parse();
    match cli.command {
        Commands::Init(args) => cmd_init::run_init(args).await,
        Commands::Run(args) => cmd_run::run_run(args).await,
        Commands::Send(args) => cmd_send::run_send(args).await,
        Commands::Chat(args) => cmd_chat::run_chat(args).await,
    }
}

pub(crate) async fn connect_with_console_plugin(
    config: ClientConfig,
    existing_state: Option<Arc<ConsoleState>>,
) -> anyhow::Result<(y2m_client_core::ClientRuntime, Arc<ConsoleState>)> {
    let state = if let Some(s) = existing_state {
        s.clear_file_transfer_state();
        s
    } else {
        Arc::new(ConsoleState::new(config.download_dir.clone()))
    };
    let mut core = ClientCore::new(config);
    core.plugin_registry_mut().register(Arc::new(ConsolePlugin { state: state.clone() }));
    let runtime = core.connect().await?;
    Ok((runtime, state))
}

pub(crate) async fn connect_with_console_plugin_with_retry(
    config: &ClientConfig,
    existing_state: Option<Arc<ConsoleState>>,
    reconnect_interval_sec: u64,
) -> anyhow::Result<(y2m_client_core::ClientRuntime, Arc<ConsoleState>)> {
    loop {
        match connect_with_console_plugin(config.clone(), existing_state.clone()).await {
            Ok(result) => return Ok(result),
            Err(e) if reconnect_interval_sec > 0 => {
                printer::cprintln!("连接失败，将在 {} 秒后重试: {}", reconnect_interval_sec.max(1), e);
                tokio::time::sleep(std::time::Duration::from_secs(reconnect_interval_sec.max(1))).await;
            }
            Err(e) => return Err(e),
        }
    }
}

fn init_logging() -> WorkerGuard {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let log_path = std::env::var("Y2M_LOG").unwrap_or_else(|_| "y2m.log".to_string());
    let file = std::fs::OpenOptions::new()
        .create(true).append(true).open(&log_path)
        .unwrap_or_else(|e| panic!("无法打开日志文件 {log_path}: {e}"));
    let (writer, guard) = tracing_appender::non_blocking(file);

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer().with_writer(writer).with_ansi(false))
        .init();

    guard
}

pub(crate) fn spawn_stdin_reader(line_tx: mpsc::UnboundedSender<String>) {
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => { if line_tx.send(line).is_err() { break; } }
                Err(_) => break,
            }
        }
    });
}
