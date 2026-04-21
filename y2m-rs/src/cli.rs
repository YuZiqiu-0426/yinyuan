use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "y2m")]
#[command(version)]
#[command(about = "Y2M CLI")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    Init(InitArgs),
    Run(RunArgs),
    Send(SendArgs),
    Chat(ChatArgs),
}

#[derive(Args, Debug)]
pub(crate) struct InitArgs {
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    #[arg(long)]
    pub(crate) server_url: Option<String>,
    #[arg(long)]
    pub(crate) group: Option<String>,
    #[arg(long)]
    pub(crate) client: Option<String>,
    #[arg(long)]
    pub(crate) token: Option<String>,
    #[arg(long)]
    pub(crate) heartbeat_interval: Option<u64>,
    #[arg(long)]
    pub(crate) download_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub(crate) struct RunArgs {
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    #[arg(long, default_value_t = 5)]
    pub(crate) reconnect_interval_sec: u64,
}

#[derive(Args, Debug)]
pub(crate) struct SendArgs {
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    #[command(subcommand)]
    pub(crate) kind: SendCommand,
}

#[derive(Subcommand, Debug)]
pub(crate) enum SendCommand {
    Text(TextArgs),
    Json(JsonArgs),
    Command(CommandArgs),
    File(FileArgs),
}

#[derive(Args, Debug)]
pub(crate) struct TextArgs {
    #[arg(long)]
    pub(crate) to: Option<String>,
    #[arg(long)]
    pub(crate) group: Option<String>,
    pub(crate) content: String,
}

#[derive(Args, Debug)]
pub(crate) struct JsonArgs {
    #[arg(long)]
    pub(crate) to: Option<String>,
    #[arg(long)]
    pub(crate) group: Option<String>,
    pub(crate) content: String,
}

#[derive(Args, Debug)]
pub(crate) struct CommandArgs {
    #[arg(long)]
    pub(crate) to: Option<String>,
    #[arg(long)]
    pub(crate) group: Option<String>,
    #[arg(long, default_value_t = 30)]
    pub(crate) timeout: u64,
    pub(crate) command: String,
}

#[derive(Args, Debug)]
pub(crate) struct FileArgs {
    #[arg(long)]
    pub(crate) to: Option<String>,
    #[arg(long)]
    pub(crate) group: Option<String>,
    #[arg(long, default_value_t = 30)]
    pub(crate) timeout: u64,
    pub(crate) path: PathBuf,
}

#[derive(Args, Debug)]
pub(crate) struct ChatArgs {
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    #[arg(long)]
    pub(crate) to: Option<String>,
    #[arg(long)]
    pub(crate) group: Option<String>,
    #[arg(long, default_value_t = 5)]
    pub(crate) reconnect_interval_sec: u64,
}
