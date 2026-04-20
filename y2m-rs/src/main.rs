use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use clap::{Args, Parser, Subcommand};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::process::Command as TokioCommand;
use tokio::sync::mpsc;
use uuid::Uuid;
use y2m_client_core::{
    build_ack_packet, build_command_result_packet, build_file_abort_event_packet,
    build_file_accept_event_packet, build_file_complete_event_packet,
    build_file_offer_event_packet, build_file_reject_event_packet, ClientConfig,
    ClientCore, ClientRuntime, IncomingServerPacket, IncomingRuntimeMessage, Plugin,
    PluginContext,
};
use y2m_common::{
    default_shell_arg, default_shell_program, AckPacket, AckStatus, BinaryChunkHeader, ErrorCode,
    ErrorPacket, EventPacket, EventType, PacketKind,
};

#[derive(Parser, Debug)]
#[command(name = "y2m")]
#[command(version)]
#[command(about = "Y2M CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Init(InitArgs),
    Run(RunArgs),
    Send(SendArgs),
    Chat(ChatArgs),
}

#[derive(Args, Debug)]
struct InitArgs {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    server_url: Option<String>,
    #[arg(long)]
    group: Option<String>,
    #[arg(long)]
    client: Option<String>,
    #[arg(long)]
    token: Option<String>,
    #[arg(long)]
    heartbeat_interval: Option<u64>,
    #[arg(long)]
    download_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct RunArgs {
    #[arg(long)]
    config: Option<PathBuf>,
    /// 断线后自动重连间隔（秒）；0 表示不重连
    #[arg(long, default_value_t = 5)]
    reconnect_interval_sec: u64,
}

#[derive(Args, Debug)]
struct SendArgs {
    #[arg(long)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    kind: SendCommand,
}

#[derive(Subcommand, Debug)]
enum SendCommand {
    Text(TextArgs),
    Json(JsonArgs),
    Command(CommandArgs),
    File(FileArgs),
}

#[derive(Args, Debug)]
struct TextArgs {
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    group: Option<String>,
    content: String,
}

#[derive(Args, Debug)]
struct JsonArgs {
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    group: Option<String>,
    content: String,
}

#[derive(Args, Debug)]
struct CommandArgs {
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    group: Option<String>,
    #[arg(long, default_value_t = 30)]
    timeout: u64,
    command: String,
}

#[derive(Args, Debug)]
struct FileArgs {
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    group: Option<String>,
    #[arg(long, default_value_t = 30)]
    timeout: u64,
    path: PathBuf,
}

#[derive(Args, Debug)]
struct ChatArgs {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    group: Option<String>,
    /// 断线后自动重连间隔（秒）；0 表示不重连
    #[arg(long, default_value_t = 5)]
    reconnect_interval_sec: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionLoopExit {
    UserQuit,
    Disconnected,
}

#[derive(Clone)]
struct ConsolePlugin {
    state: Arc<ConsoleState>,
}

#[derive(Default)]
struct ConsoleState {
    downloads_dir: PathBuf,
    files: Mutex<LocalFileStore>,
    reconnect_replays: Mutex<Vec<String>>,
}

#[derive(Default)]
struct LocalFileStore {
    transfers: HashMap<Uuid, LocalFileTransfer>,
}

#[derive(Clone)]
struct LocalFileTransfer {
    view: LocalTransferView,
    state: LocalFileState,
    peer_group: String,
    peer_client: String,
    file_name: String,
    bytes_total: u64,
    total_chunks: u32,
    sha256: String,
    save_path: Option<PathBuf>,
    source_path: Option<PathBuf>,
    chunk_size: Option<usize>,
    outgoing_bytes: Option<Arc<Vec<u8>>>,
    incoming_bytes: Vec<u8>,
    next_chunk_index: u32,
    bytes_done: u64,
    last_reported_percent: u64,
    completion_request_id: Option<String>,
}

impl LocalFileTransfer {
    fn pending_offer(
        peer_group: String,
        peer_client: String,
        file_name: String,
        save_path: PathBuf,
        bytes_total: u64,
        sha256: String,
        total_chunks: u32,
    ) -> Self {
        Self {
            view: LocalTransferView::PendingOffer,
            state: LocalFileState::PendingOffer,
            peer_group,
            peer_client,
            file_name,
            bytes_total,
            total_chunks,
            sha256,
            save_path: Some(save_path),
            source_path: None,
            chunk_size: None,
            outgoing_bytes: None,
            incoming_bytes: Vec::new(),
            next_chunk_index: 0,
            bytes_done: 0,
            last_reported_percent: 0,
            completion_request_id: None,
        }
    }

    fn outgoing(
        peer_group: String,
        peer_client: String,
        file_name: String,
        source_path: PathBuf,
        bytes_total: u64,
        total_chunks: u32,
        chunk_size: usize,
        sha256: String,
        outgoing_bytes: Arc<Vec<u8>>,
    ) -> Self {
        Self {
            view: LocalTransferView::Outgoing,
            state: LocalFileState::WaitingAccept,
            peer_group,
            peer_client,
            file_name,
            bytes_total,
            total_chunks,
            sha256,
            save_path: None,
            source_path: Some(source_path),
            chunk_size: Some(chunk_size),
            outgoing_bytes: Some(outgoing_bytes),
            incoming_bytes: Vec::new(),
            next_chunk_index: 0,
            bytes_done: 0,
            last_reported_percent: 0,
            completion_request_id: None,
        }
    }

    fn move_to_incoming(&mut self) -> Result<(), LocalFileTransitionError> {
        self.ensure_pending_offer("move pending offer to incoming")?;
        self.view = LocalTransferView::Incoming;
        self.state = LocalFileState::Receiving;
        self.incoming_bytes.clear();
        self.next_chunk_index = 0;
        self.bytes_done = 0;
        self.last_reported_percent = 0;
        Ok(())
    }

    fn is_pending_offer(&self) -> bool {
        matches!(self.state, LocalFileState::PendingOffer)
    }

    fn is_incoming(&self) -> bool {
        matches!(self.state, LocalFileState::Receiving)
    }

    fn is_outgoing(&self) -> bool {
        matches!(
            self.state,
            LocalFileState::WaitingAccept | LocalFileState::Sending | LocalFileState::WaitingAck
        )
    }

    fn is_waiting_ack(&self) -> bool {
        matches!(self.state, LocalFileState::WaitingAck)
    }

    fn ensure_pending_offer(
        &self,
        action: &'static str,
    ) -> Result<(), LocalFileTransitionError> {
        if self.is_pending_offer() {
            Ok(())
        } else {
            Err(LocalFileTransitionError::UnexpectedState {
                action,
                expected: "pending offer",
                actual: self.state,
            })
        }
    }

    fn ensure_incoming(&self, action: &'static str) -> Result<(), LocalFileTransitionError> {
        if self.is_incoming() {
            Ok(())
        } else {
            Err(LocalFileTransitionError::UnexpectedState {
                action,
                expected: "incoming transfer",
                actual: self.state,
            })
        }
    }

    fn ensure_outgoing(&self, action: &'static str) -> Result<(), LocalFileTransitionError> {
        if self.is_outgoing() {
            Ok(())
        } else {
            Err(LocalFileTransitionError::UnexpectedState {
                action,
                expected: "outgoing transfer",
                actual: self.state,
            })
        }
    }

    fn ensure_waiting_ack(&self, action: &'static str) -> Result<(), LocalFileTransitionError> {
        if self.is_waiting_ack() {
            Ok(())
        } else {
            Err(LocalFileTransitionError::UnexpectedState {
                action,
                expected: "waiting ack",
                actual: self.state,
            })
        }
    }

    fn transition_to(
        &mut self,
        expected_from: LocalFileState,
        next_state: LocalFileState,
        completion_request_id: Option<String>,
    ) -> Result<(), LocalFileTransitionError> {
        self.ensure_outgoing("transition outgoing state")?;
        if self.state != expected_from {
            return Err(LocalFileTransitionError::InvalidTransition {
                expected_from,
                actual: self.state,
                to: next_state,
            });
        }
        self.state = next_state;
        self.completion_request_id = completion_request_id;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalFileTransitionError {
    UnexpectedState {
        action: &'static str,
        expected: &'static str,
        actual: LocalFileState,
    },
    InvalidTransition {
        expected_from: LocalFileState,
        actual: LocalFileState,
        to: LocalFileState,
    },
}

struct FileAcceptInfo {
    save_path: Option<String>,
}

enum FileTransferFailureReason {
    Disconnected,
    Protocol(String),
    MissingChunks {
        expected_chunks: u32,
        received_chunks: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalTransferView {
    PendingOffer,
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalFileState {
    PendingOffer,
    Receiving,
    WaitingAccept,
    Sending,
    WaitingAck,
}

struct LocalFileEntry {
    file_id: Uuid,
    view: LocalTransferView,
    state: LocalFileState,
    peer_group: String,
    peer_client: String,
    file_name: String,
    bytes_done: u64,
    bytes_total: u64,
    save_path: Option<PathBuf>,
}

impl FileTransferFailureReason {
    fn reason_text(&self) -> String {
        match self {
            Self::Disconnected => "连接中断，请等待对方在线后重新发送".to_string(),
            Self::Protocol(reason) => reason.clone(),
            Self::MissingChunks {
                expected_chunks,
                received_chunks,
            } => format!(
                "分片不完整，expected_chunks={}, received_chunks={}",
                expected_chunks, received_chunks
            ),
        }
    }
}

impl ConsoleState {
    fn new(downloads_dir: Option<PathBuf>) -> Self {
        let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            downloads_dir: downloads_dir.unwrap_or_else(|| base.join("downloads")),
            files: Mutex::new(LocalFileStore::default()),
            reconnect_replays: Mutex::new(Vec::new()),
        }
    }

    fn transfer_failure_line(
        view: LocalTransferView,
        file_id: Uuid,
        file_name: &str,
        reason: &FileTransferFailureReason,
    ) -> String {
        let reason = reason.reason_text();
        match view {
            LocalTransferView::PendingOffer => {
                format!("待确认文件已失效: id={}, name={}, reason={}", file_id, file_name, reason)
            }
            LocalTransferView::Incoming => {
                format!("接收失败: id={}, name={}, reason={}", file_id, file_name, reason)
            }
            LocalTransferView::Outgoing => {
                format!("发送失败: id={}, name={}, reason={}", file_id, file_name, reason)
            }
        }
    }

    fn queue_reconnect_failure(
        &self,
        view: LocalTransferView,
        file_id: Uuid,
        file_name: &str,
        reason: FileTransferFailureReason,
    ) {
        self.reconnect_replays
            .lock()
            .expect("lock reconnect replays")
            .push(Self::transfer_failure_line(view, file_id, file_name, &reason));
    }

    fn drain_local_file_entries_for_reconnect(&self) -> Vec<LocalFileEntry> {
        let transfers = {
            let mut files = self.files.lock().expect("lock local file store");
            std::mem::take(&mut files.transfers)
                .into_iter()
                .collect::<Vec<_>>()
        };
        transfers
            .iter()
            .map(|(file_id, transfer)| Self::local_entry_from_transfer(*file_id, transfer))
            .collect()
    }

    fn local_entry_from_transfer(file_id: Uuid, transfer: &LocalFileTransfer) -> LocalFileEntry {
        LocalFileEntry {
            file_id,
            view: transfer.view,
            state: transfer.state,
            peer_group: transfer.peer_group.clone(),
            peer_client: transfer.peer_client.clone(),
            file_name: transfer.file_name.clone(),
            bytes_done: transfer.bytes_done,
            bytes_total: transfer.bytes_total,
            save_path: transfer.save_path.clone(),
        }
    }

    fn insert_pending_offer(&self, file_id: Uuid, transfer: LocalFileTransfer) {
        self.files
            .lock()
            .expect("lock local file store")
            .transfers
            .insert(file_id, transfer);
    }

    fn take_pending_offer(&self, file_id: Uuid) -> Option<LocalFileTransfer> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.remove(&file_id)?;
        if transfer.ensure_pending_offer("take pending offer").is_ok() {
            Some(transfer)
        } else {
            files.transfers.insert(file_id, transfer);
            None
        }
    }

    fn take_incoming_file(&self, file_id: Uuid) -> Option<LocalFileTransfer> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.remove(&file_id)?;
        if transfer.ensure_incoming("take incoming transfer").is_ok() {
            Some(transfer)
        } else {
            files.transfers.insert(file_id, transfer);
            None
        }
    }

    fn insert_outgoing_file(&self, file_id: Uuid, transfer: LocalFileTransfer) {
        self.files
            .lock()
            .expect("lock local file store")
            .transfers
            .insert(file_id, transfer);
    }

    fn take_outgoing_file(&self, file_id: Uuid) -> Option<LocalFileTransfer> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.remove(&file_id)?;
        if transfer.ensure_outgoing("take outgoing transfer").is_ok() {
            Some(transfer)
        } else {
            files.transfers.insert(file_id, transfer);
            None
        }
    }

    fn has_outgoing_file(&self, file_id: Uuid) -> bool {
        self.files
            .lock()
            .expect("lock local file store")
            .transfers
            .get(&file_id)
            .is_some_and(LocalFileTransfer::is_outgoing)
    }

    fn take_outgoing_by_completion_request_id(
        &self,
        request_id: &str,
    ) -> Option<(Uuid, LocalFileTransfer)> {
        let mut files = self.files.lock().expect("lock local file store");
        let file_id = files.transfers.iter().find_map(|(file_id, transfer)| {
            (transfer.completion_request_id.as_deref() == Some(request_id)
                && transfer
                    .ensure_waiting_ack("take outgoing transfer by completion request id")
                    .is_ok())
                .then_some(*file_id)
        })?;
        let transfer = files.transfers.remove(&file_id)?;
        if transfer
            .ensure_waiting_ack("take outgoing transfer by completion request id")
            .is_ok()
        {
            Some((file_id, transfer))
        } else {
            files.transfers.insert(file_id, transfer);
            None
        }
    }

    fn mutate_incoming_file<R>(
        &self,
        file_id: Uuid,
        f: impl FnOnce(&mut LocalFileTransfer) -> R,
    ) -> Option<R> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.get_mut(&file_id)?;
        if transfer.ensure_incoming("mutate incoming transfer").is_ok() {
            Some(f(transfer))
        } else {
            None
        }
    }

    fn apply_incoming_chunk(
        &self,
        header: &BinaryChunkHeader,
        payload: &[u8],
    ) -> Result<Option<(String, u64, u64)>, FileTransferFailureReason> {
        let update = self.mutate_incoming_file(header.file_id, |pending| {
            if header.total_chunks != pending.total_chunks {
                return Err(FileTransferFailureReason::Protocol(format!(
                    "chunk total mismatch: expected {}, got {}",
                    pending.total_chunks, header.total_chunks
                )));
            }
            if header.chunk_index >= pending.total_chunks {
                return Err(FileTransferFailureReason::Protocol(format!(
                    "chunk index out of range: expected < {}, got {}",
                    pending.total_chunks, header.chunk_index
                )));
            }
            if header.chunk_index != pending.next_chunk_index {
                return Err(FileTransferFailureReason::Protocol(format!(
                    "chunk sequence mismatch: expected {}, got {}",
                    pending.next_chunk_index, header.chunk_index
                )));
            }

            pending.incoming_bytes.extend_from_slice(payload);
            pending.next_chunk_index += 1;
            pending.bytes_done = pending.incoming_bytes.len() as u64;

            let received_bytes = pending.bytes_done;
            if received_bytes > pending.bytes_total {
                return Err(FileTransferFailureReason::Protocol(format!(
                    "received bytes exceed expected size: expected {}, got {}",
                    pending.bytes_total, received_bytes
                )));
            }

            let total_bytes = pending.bytes_total;
            let percent = if total_bytes == 0 {
                100
            } else {
                received_bytes.saturating_mul(100) / total_bytes
            };
            let should_report = pending.total_chunks <= 10
                || percent >= pending.last_reported_percent + 10
                || percent == 100
                || header.chunk_index + 1 == header.total_chunks;
            if should_report {
                pending.last_reported_percent = percent;
                Ok(Some((pending.file_name.clone(), pending.bytes_done, total_bytes)))
            } else {
                Ok(None)
            }
        });

        match update {
            Some(result) => result,
            None => Ok(None),
        }
    }

    fn mutate_outgoing_file<R>(
        &self,
        file_id: Uuid,
        f: impl FnOnce(&mut LocalFileTransfer) -> R,
    ) -> Option<R> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.get_mut(&file_id)?;
        if transfer.ensure_outgoing("mutate outgoing transfer").is_ok() {
            Some(f(transfer))
        } else {
            None
        }
    }

    fn move_pending_offer_to_incoming(&self, file_id: Uuid) -> Option<LocalFileTransfer> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.get_mut(&file_id)?;
        if transfer.move_to_incoming().is_err() {
            return None;
        }
        Some(transfer.clone())
    }

    fn start_outgoing_waiting_accept(&self, file_id: Uuid, transfer: LocalFileTransfer) {
        self.insert_outgoing_file(file_id, transfer);
    }

    fn update_outgoing_phase(
        &self,
        file_id: Uuid,
        expected_from: LocalFileState,
        phase: LocalFileState,
        completion_request_id: Option<String>,
    ) -> Option<LocalFileTransfer> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.get_mut(&file_id)?;
        if transfer
            .transition_to(expected_from, phase, completion_request_id)
            .is_err()
        {
            return None;
        }
        Some(transfer.clone())
    }

    /// 重连前清空文件传输相关状态，避免旧 `fileId` 与服务器不一致；
    /// 清理结果在重连成功后回放给本地用户。
    fn clear_file_transfer_state(&self) {
        for entry in self.drain_local_file_entries_for_reconnect() {
            self.queue_reconnect_failure(
                entry.view,
                entry.file_id,
                &entry.file_name,
                FileTransferFailureReason::Disconnected,
            );
        }
    }

    fn replay_after_reconnect(&self) {
        let messages = {
            let mut replays = self
                .reconnect_replays
                .lock()
                .expect("lock reconnect replays");
            std::mem::take(&mut *replays)
        };
        for message in messages {
            println!("{message}");
        }
    }

    fn handle_file_offer(&self, ctx: &PluginContext, packet: &EventPacket) -> anyhow::Result<()> {
        let file_id = parse_file_id(packet)?;
        let file_name = packet
            .payload
            .metadata
            .get("fileName")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown.bin")
            .to_string();
        let expected_size = packet
            .payload
            .metadata
            .get("fileSize")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let expected_sha256 = packet
            .payload
            .metadata
            .get("sha256")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let total_chunks = packet
            .payload
            .metadata
            .get("totalChunks")
            .and_then(|value| value.as_u64())
            .unwrap_or(1) as u32;
        let source_group = packet
            .source
            .as_ref()
            .and_then(|endpoint| endpoint.group_name.clone())
            .unwrap_or_else(|| ctx.identity.group_name.clone());
        let source_client = packet
            .source
            .as_ref()
            .and_then(|endpoint| endpoint.client_name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let save_path = ensure_unique_path(self.downloads_dir.join(&file_name));

        self.insert_pending_offer(
            file_id,
            LocalFileTransfer::pending_offer(
                source_group.clone(),
                source_client.clone(),
                file_name.clone(),
                save_path.clone(),
                expected_size,
                expected_sha256,
                total_chunks,
            ),
        );

        println!(
            "收到文件请求: id={}, from=[{}][{}], name={}, size={} bytes, 保存到={}",
            file_id,
            source_group,
            source_client,
            file_name,
            expected_size,
            save_path.display()
        );
        println!("输入 /accept {file_id} 接收，或 /reject {file_id} 拒绝");
        println!("也可以输入 /files 查看当前文件列表");
        Ok(())
    }

    fn accept_pending_offer(
        &self,
        runtime: &ClientRuntime,
        file_id: Uuid,
    ) -> anyhow::Result<bool> {
        let offer = self.move_pending_offer_to_incoming(file_id);
        let Some(offer) = offer else {
            return Ok(false);
        };

        let accept = build_file_accept_event_packet(
            runtime.identity(),
            Some(offer.peer_group),
            Some(offer.peer_client),
            file_id,
            offer
                .save_path
                .as_ref()
                .expect("pending offer save path")
                .display()
                .to_string(),
        );
        runtime.connection().send_json_packet(&accept)?;
        println!(
            "已接受文件: id={}, 保存到={}, 等待分片传输...",
            file_id,
            offer
                .save_path
                .as_ref()
                .expect("pending offer save path")
                .display()
        );
        Ok(true)
    }

    fn reject_pending_offer(
        &self,
        runtime: &ClientRuntime,
        file_id: Uuid,
    ) -> anyhow::Result<bool> {
        let offer = self.take_pending_offer(file_id);
        let Some(offer) = offer else {
            return Ok(false);
        };

        let reject = build_file_reject_event_packet(
            runtime.identity(),
            Some(offer.peer_group),
            Some(offer.peer_client),
            file_id,
            "rejected by user",
        );
        runtime.connection().send_json_packet(&reject)?;
        println!("已拒绝文件: id={}, name={}", file_id, offer.file_name);
        Ok(true)
    }

    fn start_outgoing_file(
        self: &Arc<Self>,
        runtime: &ClientRuntime,
        path: &Path,
        target_group_name: Option<String>,
        target_client_name: Option<String>,
    ) -> anyhow::Result<()> {
        let target_client = target_client_name
            .ok_or_else(|| anyhow::anyhow!("file 仅支持单播，请指定目标客户端"))?;
        let metadata = fs::metadata(path)?;
        if !metadata.is_file() {
            anyhow::bail!("目标不是文件: {}", path.display());
        }

        let bytes = Arc::new(fs::read(path)?);
        let file_id = Uuid::new_v4();
        let file_name = path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown.bin".to_string());
        let file_size = bytes.len() as u64;
        let chunk_size = 256 * 1024usize;
        let total_chunks = bytes.len().div_ceil(chunk_size) as u32;
        let target_group = target_group_name
            .unwrap_or_else(|| runtime.identity().group_name.clone());
        let content_type = guess_content_type(path);
        let sha256 = sha256_hex(bytes.as_slice());

        self.start_outgoing_waiting_accept(
            file_id,
            LocalFileTransfer::outgoing(
                target_group.clone(),
                target_client.clone(),
                file_name.clone(),
                path.to_path_buf(),
                file_size,
                total_chunks.max(1),
                chunk_size,
                sha256.clone(),
                bytes.clone(),
            ),
        );

        let offer_packet = build_file_offer_event_packet(
            runtime.identity(),
            Some(target_group.clone()),
            Some(target_client.clone()),
            file_id,
            file_name.clone(),
            file_size,
            content_type,
            sha256,
            chunk_size as u32,
            total_chunks.max(1),
        );

        println!(
            "开始发送文件: id={}, name={}, size={}, chunks={}, to=[{}][{}]",
            file_id,
            file_name,
            format_bytes(file_size),
            total_chunks.max(1),
            target_group,
            target_client
        );
        runtime.connection().send_json_packet(&offer_packet)?;
        println!("已发送文件申请，等待对方确认...");
        Ok(())
    }

    async fn handle_outgoing_file_accept(
        self: &Arc<Self>,
        ctx: &PluginContext,
        packet: &EventPacket,
    ) -> anyhow::Result<()> {
        let file_id = parse_file_id(packet)?;
        let save_path = packet
            .payload
            .metadata
            .get("savePath")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned);
        let maybe_transfer = self
            .update_outgoing_phase(
                file_id,
                LocalFileState::WaitingAccept,
                LocalFileState::Sending,
                None,
            )
            .map(|transfer| {
                (
                    transfer.peer_group,
                    transfer.peer_client,
                    transfer.file_name,
                    transfer.source_path.expect("outgoing source path"),
                    transfer.bytes_total,
                    transfer.total_chunks,
                    transfer.chunk_size.expect("outgoing chunk size"),
                    transfer.sha256,
                    transfer.outgoing_bytes.expect("outgoing bytes"),
                )
            });

        let Some((
            target_group,
            target_client,
            file_name,
            source_path,
            file_size,
            total_chunks,
            chunk_size,
            sha256,
            bytes,
        )) = maybe_transfer
        else {
            return Ok(());
        };

        if let Some(save_path) = save_path {
            println!("对方已接受，目标保存路径: {}", save_path);
        } else {
            println!("对方已接受，开始发送分片...");
        }

        let state = self.clone();
        let connection = ctx.connection.clone();
        let identity = ctx.identity.clone();
        tokio::spawn(async move {
            if let Err(error) = state
                .clone()
                .run_outgoing_file_send(
                    connection,
                    identity,
                    file_id,
                    target_group,
                    target_client,
                    file_name,
                    source_path,
                    file_size,
                    total_chunks,
                    chunk_size,
                    sha256,
                    bytes,
                )
                .await
            {
                println!("文件发送失败: {}", error);
                state.take_outgoing_file(file_id);
            }
        });
        Ok(())
    }

    fn fail_incoming_transfer(
        &self,
        runtime: &ClientRuntime,
        file_id: Uuid,
        reason: FileTransferFailureReason,
    ) -> anyhow::Result<()> {
        let pending = self.take_incoming_file(file_id);
        let Some(pending) = pending else {
            return Ok(());
        };

        let reason_text = reason.reason_text();
        println!(
            "文件接收失败: id={}, name={}, reason={}",
            file_id, pending.file_name, reason_text
        );
        let abort = build_file_abort_event_packet(
            runtime.identity(),
            Some(pending.peer_group),
            Some(pending.peer_client),
            file_id,
            &reason_text,
        );
        runtime.connection().send_json_packet(&abort)?;
        Ok(())
    }

    fn handle_binary_frame(&self, runtime: &ClientRuntime, frame: Vec<u8>) -> anyhow::Result<()> {
        let (header, payload) =
            BinaryChunkHeader::decode(&frame).ok_or_else(|| anyhow::anyhow!("invalid binary chunk"))?;
        match self.apply_incoming_chunk(&header, payload) {
            Ok(Some((file_name, received_bytes, total_bytes))) => {
                let percent = if total_bytes == 0 {
                    100
                } else {
                    received_bytes.saturating_mul(100) / total_bytes
                };
                println!(
                    "接收进度: file={}, {}% ({}/{}), chunk {}/{}",
                    file_name,
                    percent,
                    format_bytes(received_bytes),
                    format_bytes(total_bytes),
                    header.chunk_index + 1,
                    header.total_chunks
                );
            }
            Ok(None) => {}
            Err(reason) => {
                self.fail_incoming_transfer(runtime, header.file_id, reason)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn handle_file_complete(
        &self,
        ctx: &PluginContext,
        packet: &EventPacket,
    ) -> anyhow::Result<()> {
        let file_id = parse_file_id(packet)?;
        let pending = self.take_incoming_file(file_id);
        let Some(pending) = pending else {
            return Ok(());
        };

        let actual_sha256 = sha256_hex(&pending.incoming_bytes);
        let status = if pending.next_chunk_index != pending.total_chunks {
            let reason = FileTransferFailureReason::MissingChunks {
                expected_chunks: pending.total_chunks,
                received_chunks: pending.next_chunk_index,
            };
            println!(
                "文件接收失败: id={}, name={}, reason={}",
                file_id,
                pending.file_name,
                reason.reason_text()
            );
            AckStatus::Rejected
        } else if pending.incoming_bytes.len() as u64 == pending.bytes_total
            && (pending.sha256.is_empty() || actual_sha256 == pending.sha256)
        {
            let save_path = pending.save_path.as_ref().expect("incoming save path");
            if let Some(parent) = save_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(save_path, &pending.incoming_bytes)?;
            println!("文件已保存: {}", save_path.display());
            AckStatus::Ok
        } else {
            println!(
                "文件校验失败: name={}, expected_size={}, actual_size={}, expected_sha256={}, actual_sha256={}",
                pending.file_name,
                pending.bytes_total,
                pending.incoming_bytes.len(),
                pending.sha256,
                actual_sha256
            );
            AckStatus::Rejected
        };

        let ack = build_ack_packet(
            &ctx.identity,
            Some(pending.peer_group),
            Some(pending.peer_client),
            packet.request_id.clone(),
            PacketKind::Event,
            Some(EventType::FileComplete),
            status,
        );
        ctx.connection.send_json_packet(&ack)?;
        Ok(())
    }

    fn handle_file_abort(&self, packet: &EventPacket) -> anyhow::Result<()> {
        let file_id = parse_file_id(packet)?;
        let reason = packet
            .payload
            .metadata
            .get("reason")
            .and_then(|value| value.as_str())
            .unwrap_or("未提供原因");
        let removed_outgoing = self.take_outgoing_file(file_id);
        self.take_pending_offer(file_id);
        let removed_incoming = self.take_incoming_file(file_id);
        if let Some(transfer) = removed_outgoing {
            println!(
                "发送已取消: id={}, name={}, reason={}",
                file_id, transfer.file_name, reason
            );
        }
        if let Some(transfer) = removed_incoming {
            println!(
                "接收已取消: id={}, name={}, reason={}",
                file_id, transfer.file_name, reason
            );
        }
        Ok(())
    }

    fn handle_file_reject(&self, packet: &EventPacket) -> anyhow::Result<()> {
        let file_id = parse_file_id(packet)?;
        let reason = packet
            .payload
            .metadata
            .get("reason")
            .and_then(|value| value.as_str())
            .unwrap_or("未提供原因");
        let removed_outgoing = self.take_outgoing_file(file_id);
        self.take_pending_offer(file_id);
        self.take_incoming_file(file_id);
        if let Some(transfer) = removed_outgoing {
            println!(
                "对方拒绝接收文件: id={}, name={}, reason={}",
                file_id, transfer.file_name, reason
            );
        }
        Ok(())
    }

    fn abort_transfer(&self, runtime: &ClientRuntime, file_id: Uuid) -> anyhow::Result<bool> {
        if let Some(transfer) = self.take_outgoing_file(file_id) {
            let packet = build_file_abort_event_packet(
                runtime.identity(),
                Some(transfer.peer_group.clone()),
                Some(transfer.peer_client.clone()),
                file_id,
                "aborted by user",
            );
            runtime.connection().send_json_packet(&packet)?;
            println!("已取消发送: id={}, name={}", file_id, transfer.file_name);
            return Ok(true);
        }

        if let Some(transfer) = self.take_incoming_file(file_id) {
            let packet = build_file_abort_event_packet(
                runtime.identity(),
                Some(transfer.peer_group.clone()),
                Some(transfer.peer_client.clone()),
                file_id,
                "aborted by user",
            );
            runtime.connection().send_json_packet(&packet)?;
            println!("已取消接收: id={}, name={}", file_id, transfer.file_name);
            return Ok(true);
        }

        if let Some(offer) = self.take_pending_offer(file_id) {
            let packet = build_file_abort_event_packet(
                runtime.identity(),
                Some(offer.peer_group.clone()),
                Some(offer.peer_client.clone()),
                file_id,
                "aborted by user",
            );
            runtime.connection().send_json_packet(&packet)?;
            println!("已取消待确认文件: id={}, name={}", file_id, offer.file_name);
            return Ok(true);
        }

        Ok(false)
    }

    fn handle_ack(&self, packet: &AckPacket) {
        if packet.payload.ref_type != Some(EventType::FileComplete) {
            return;
        }

        let Some((file_id, transfer)) =
            self.take_outgoing_by_completion_request_id(packet.request_id.as_str())
        else {
            return;
        };

        match packet.payload.status {
            AckStatus::Ok => {
                println!(
                    "文件发送完成: {} ({})",
                    transfer
                        .source_path
                        .as_ref()
                        .expect("outgoing source path")
                        .display(),
                    transfer.file_name
                );
            }
            AckStatus::Rejected => {
                println!("对方拒绝确认文件: id={}, name={}", file_id, transfer.file_name);
            }
        }
    }

    fn handle_server_error(&self, packet: &ErrorPacket) {
        let maybe_file_id = packet
            .payload
            .details
            .get("fileId")
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok());

        if let Some(file_id) = maybe_file_id {
            if let Some(transfer) = self.take_outgoing_file(file_id) {
                println!(
                    "文件发送失败: id={}, name={}, code={:?}, message={}",
                    file_id, transfer.file_name, packet.payload.code, packet.payload.message
                );
                return;
            }
        }

        if matches!(
            packet.payload.code,
            ErrorCode::FileTooLarge | ErrorCode::FileTransferNotAccepted | ErrorCode::InvalidMessage
        ) {
            println!(
                "服务端错误: code={:?}, message={}",
                packet.payload.code, packet.payload.message
            );
        }
    }

    async fn handle_command_event(
        &self,
        ctx: &PluginContext,
        packet: &EventPacket,
    ) -> anyhow::Result<()> {
        let command = packet
            .payload
            .content
            .as_str()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| packet.payload.content.to_string());
        let timeout_sec = packet
            .payload
            .metadata
            .get("timeoutSec")
            .and_then(|value| value.as_u64())
            .unwrap_or(30)
            .max(1);
        let target_group = packet
            .source
            .as_ref()
            .and_then(|endpoint| endpoint.group_name.clone());
        let target_client = packet
            .source
            .as_ref()
            .and_then(|endpoint| endpoint.client_name.clone());

        println!(
            "收到命令请求: from=[{}][{}], timeout={}s, command={}",
            target_group.as_deref().unwrap_or("unknown"),
            target_client.as_deref().unwrap_or("unknown"),
            timeout_sec,
            command
        );

        let started_at = Instant::now();
        let mut process = TokioCommand::new(default_shell_program());
        process
            .arg(default_shell_arg())
            .arg(&command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let (exit_code, stdout, stderr) =
            match tokio::time::timeout(Duration::from_secs(timeout_sec), process.output()).await {
                Ok(Ok(output)) => (
                    output.status.code().unwrap_or(-1),
                    String::from_utf8_lossy(&output.stdout).to_string(),
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ),
                Ok(Err(error)) => (
                    -1,
                    String::new(),
                    format!("failed to execute command: {error}"),
                ),
                Err(_) => (
                    -1,
                    String::new(),
                    format!("command timed out after {timeout_sec}s"),
                ),
            };
        let duration_ms = started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;

        let packet = build_command_result_packet(
            &ctx.identity,
            target_group,
            target_client,
            packet.request_id.clone(),
            exit_code,
            stdout,
            stderr,
            duration_ms,
        );
        ctx.connection.send_json_packet(&packet)?;
        Ok(())
    }

    async fn run_outgoing_file_send(
        self: Arc<Self>,
        connection: y2m_client_core::ClientConnection,
        identity: y2m_client_core::ClientIdentity,
        file_id: Uuid,
        target_group: String,
        target_client: String,
        file_name: String,
        source_path: PathBuf,
        file_size: u64,
        total_chunks: u32,
        chunk_size: usize,
        sha256: String,
        bytes: Arc<Vec<u8>>,
    ) -> anyhow::Result<()> {
        for (chunk_index, chunk) in bytes.chunks(chunk_size).enumerate() {
            if !self.has_outgoing_file(file_id) {
                return Ok(());
            }

            let frame = BinaryChunkHeader::new(
                file_id,
                chunk_index as u32,
                total_chunks.max(1),
                chunk.len() as u32,
            )
            .encode_with_payload(chunk);
            connection.send_binary(frame)?;

            self.update_outgoing_progress(file_id, file_size, chunk_index + 1, chunk_size);
            tokio::task::yield_now().await;
        }

        if !self.has_outgoing_file(file_id) {
            return Ok(());
        }

        if file_size == 0 {
            println!("发送进度: 100% (0 B/0 B), chunk 0/1");
        }

        let complete_packet = build_file_complete_event_packet(
            &identity,
            Some(target_group),
            Some(target_client),
            file_id,
            file_size,
            sha256,
        );
        let complete_request_id = complete_packet.request_id.clone();
        connection.send_json_packet(&complete_packet)?;
        if self
            .update_outgoing_phase(
                file_id,
                LocalFileState::Sending,
                LocalFileState::WaitingAck,
                Some(complete_request_id),
            )
            .is_none()
        {
            return Ok(());
        }
        println!(
            "分片发送完成，等待对方校验确认... id={}, file={}",
            file_id, file_name
        );
        let _ = source_path;
        Ok(())
    }

    fn update_outgoing_progress(
        &self,
        file_id: Uuid,
        total_bytes: u64,
        chunk_index: usize,
        chunk_size: usize,
    ) {
        let update = self.mutate_outgoing_file(file_id, |transfer| {
            let sent_bytes = (chunk_index * chunk_size)
                .min(
                    transfer
                        .outgoing_bytes
                        .as_ref()
                        .expect("outgoing bytes")
                        .len(),
                ) as u64;
            transfer.bytes_done = sent_bytes;
            let percent = if total_bytes == 0 {
                100
            } else {
                sent_bytes.saturating_mul(100) / total_bytes
            };
            let should_report = transfer.total_chunks <= 10
                || percent >= transfer.last_reported_percent + 10
                || percent == 100;
            if should_report {
                transfer.last_reported_percent = percent;
                Some((sent_bytes, transfer.total_chunks, percent))
            } else {
                None
            }
        });
        let Some(Some((sent_bytes, total_chunks, percent))) = update else {
            return;
        };
        println!(
            "发送进度: {}% ({}/{}), chunk {}/{}",
            percent,
            format_bytes(sent_bytes),
            format_bytes(total_bytes),
            chunk_index,
            total_chunks
        );
    }

    fn print_file_queue(&self) {
        let entries = self.snapshot_local_file_entries();
        if entries.is_empty() {
            println!("当前没有待处理文件");
            return;
        }

        let pending: Vec<_> = entries
            .iter()
            .filter(|entry| matches!(entry.view, LocalTransferView::PendingOffer))
            .collect();
        if !pending.is_empty() {
            println!("待确认文件:");
            for entry in pending {
                println!(
                    "  id={}, 状态={}, from=[{}][{}], name={}, size={} bytes, 保存到={}",
                    entry.file_id,
                    entry.state.label(),
                    entry.peer_group,
                    entry.peer_client,
                    entry.file_name,
                    entry.bytes_total,
                    entry
                        .save_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
            }
        }

        let incoming: Vec<_> = entries
            .iter()
            .filter(|entry| matches!(entry.view, LocalTransferView::Incoming))
            .collect();
        if !incoming.is_empty() {
            println!("接收中文件:");
            for entry in incoming {
                println!(
                    "  id={}, 状态={}, from=[{}][{}], name={}, 进度={}/{} bytes, 保存到={}",
                    entry.file_id,
                    entry.state.label(),
                    entry.peer_group,
                    entry.peer_client,
                    entry.file_name,
                    entry.bytes_done,
                    entry.bytes_total,
                    entry
                        .save_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
            }
        }

        let outgoing: Vec<_> = entries
            .iter()
            .filter(|entry| matches!(entry.view, LocalTransferView::Outgoing))
            .collect();
        if !outgoing.is_empty() {
            println!("外发文件:");
            for entry in outgoing {
                println!(
                    "  id={}, 状态={}, to=[{}][{}], name={}, 进度={}/{} bytes",
                    entry.file_id,
                    entry.state.label(),
                    entry.peer_group,
                    entry.peer_client,
                    entry.file_name,
                    entry.bytes_done,
                    entry.bytes_total
                );
            }
        }
    }

    fn snapshot_local_file_entries(&self) -> Vec<LocalFileEntry> {
        let files = self.files.lock().expect("lock local file store");

        let mut entries = Vec::with_capacity(files.transfers.len());
        entries.extend(
            files
                .transfers
                .iter()
                .map(|(file_id, transfer)| Self::local_entry_from_transfer(*file_id, transfer)),
        );
        entries.sort_by(|left, right| left.file_id.cmp(&right.file_id));
        entries
    }
}

impl LocalFileState {
    fn label(self) -> &'static str {
        match self {
            Self::PendingOffer => "待确认",
            Self::Receiving => "接收中",
            Self::WaitingAccept => "等待对方接受",
            Self::Sending => "发送中",
            Self::WaitingAck => "等待对方确认",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pending_transfer() -> LocalFileTransfer {
        LocalFileTransfer::pending_offer(
            "g".to_string(),
            "c".to_string(),
            "a.txt".to_string(),
            PathBuf::from("downloads/a.txt"),
            12,
            "sha".to_string(),
            1,
        )
    }

    fn make_outgoing_transfer() -> LocalFileTransfer {
        LocalFileTransfer::outgoing(
            "g".to_string(),
            "c".to_string(),
            "a.txt".to_string(),
            PathBuf::from("a.txt"),
            12,
            1,
            1024,
            "sha".to_string(),
            Arc::new(vec![1, 2, 3]),
        )
    }

    #[test]
    fn move_to_incoming_returns_explicit_error_for_wrong_state() {
        let mut transfer = make_outgoing_transfer();

        let error = transfer.move_to_incoming().expect_err("should reject wrong state");

        assert_eq!(
            error,
            LocalFileTransitionError::UnexpectedState {
                action: "move pending offer to incoming",
                expected: "pending offer",
                actual: LocalFileState::WaitingAccept,
            }
        );
    }

    #[test]
    fn transition_to_returns_explicit_error_for_invalid_from_state() {
        let mut transfer = make_outgoing_transfer();
        transfer.state = LocalFileState::WaitingAccept;

        let error = transfer
            .transition_to(LocalFileState::Sending, LocalFileState::WaitingAck, None)
            .expect_err("should reject invalid transition");

        assert_eq!(
            error,
            LocalFileTransitionError::InvalidTransition {
                expected_from: LocalFileState::Sending,
                actual: LocalFileState::WaitingAccept,
                to: LocalFileState::WaitingAck,
            }
        );
    }

    #[test]
    fn move_to_incoming_clears_progress_and_switches_state() {
        let mut transfer = make_pending_transfer();
        transfer.bytes_done = 9;
        transfer.last_reported_percent = 70;

        transfer
            .move_to_incoming()
            .expect("pending offer should move to incoming");

        assert_eq!(transfer.view, LocalTransferView::Incoming);
        assert_eq!(transfer.state, LocalFileState::Receiving);
        assert_eq!(transfer.bytes_done, 0);
        assert_eq!(transfer.last_reported_percent, 0);
    }
}

const CONSOLE_EVENTS: &[EventType] = &[
    EventType::Text,
    EventType::Command,
    EventType::Json,
    EventType::CommandResult,
    EventType::FileOffer,
    EventType::FileAccept,
    EventType::FileReject,
    EventType::FileComplete,
    EventType::FileAbort,
];

#[async_trait]
impl Plugin for ConsolePlugin {
    fn name(&self) -> &'static str {
        "console"
    }

    fn supports(&self) -> &'static [EventType] {
        CONSOLE_EVENTS
    }

    async fn on_event(
        &self,
        ctx: &PluginContext,
        packet: &EventPacket,
    ) -> anyhow::Result<()> {
        let from = packet
            .source
            .as_ref()
            .and_then(|endpoint| endpoint.client_name.as_deref())
            .unwrap_or("unknown");
        let group = packet
            .source
            .as_ref()
            .and_then(|endpoint| endpoint.group_name.as_deref())
            .unwrap_or("unknown");

        match packet.payload.event_type {
            EventType::Text => {
                let content = packet
                    .payload
                    .content
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| packet.payload.content.to_string());
                println!("[{group}][{from}] {content}");
            }
            EventType::Json => {
                println!("[{group}][{from}] {}", packet.payload.content);
            }
            EventType::CommandResult => {
                println!("[{group}][{from}] command_result {}", packet.payload.metadata);
            }
            EventType::FileOffer => {
                println!("[{group}][{from}] file_offer {}", packet.payload.metadata);
                self.state.handle_file_offer(ctx, packet)?;
            }
            EventType::FileAccept => {
                println!("[{group}][{from}] file_accept {}", packet.payload.metadata);
                self.state.handle_outgoing_file_accept(ctx, packet).await?;
            }
            EventType::FileReject => {
                println!("[{group}][{from}] file_reject {}", packet.payload.metadata);
                self.state.handle_file_reject(packet)?;
            }
            EventType::FileComplete => {
                println!("[{group}][{from}] file_complete {}", packet.payload.metadata);
                self.state.handle_file_complete(ctx, packet)?;
            }
            EventType::FileAbort => {
                println!("[{group}][{from}] file_abort {}", packet.payload.metadata);
                self.state.handle_file_abort(packet)?;
            }
            EventType::Command => {
                self.state.handle_command_event(ctx, packet).await?;
            }
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt().try_init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => run_init(args).await,
        Commands::Run(args) => run_run(args).await,
        Commands::Send(args) => run_send(args).await,
        Commands::Chat(args) => run_chat(args).await,
    }
}

async fn run_init(args: InitArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(args.config);
    let mut config = load_or_default_config(&config_path)?;

    if let Some(server_url) = args.server_url {
        config.server_url = server_url;
    }
    if let Some(group) = args.group {
        config.group_name = Some(group);
    }
    if let Some(client) = args.client {
        config.client_name = Some(client);
    }
    if let Some(token) = args.token {
        config.token = Some(token);
    }
    if let Some(heartbeat_interval) = args.heartbeat_interval {
        config.heartbeat_interval_override_sec = Some(heartbeat_interval);
    }
    if let Some(download_dir) = args.download_dir {
        config.download_dir = Some(download_dir);
    }

    config.save_to_path(&config_path)?;
    println!("配置已保存到 {}", config_path.display());
    Ok(())
}

async fn run_send(args: SendArgs) -> anyhow::Result<()> {
    match args.kind {
        SendCommand::Text(text) => run_send_text(args.config, text).await,
        SendCommand::Json(json) => run_send_json(args.config, json).await,
        SendCommand::Command(command) => run_send_command(args.config, command).await,
        SendCommand::File(file) => run_send_file(args.config, file).await,
    }
}

async fn run_send_text(config: Option<PathBuf>, args: TextArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(config);
    let config = load_or_default_config(&config_path)?;
    let (runtime, _state) = connect_with_console_plugin(config, None).await?;

    runtime.send_text(args.group.clone(), args.to.clone(), args.content.clone())?;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let group = args
        .group
        .unwrap_or_else(|| runtime.identity().group_name.clone());
    let target = args.to.unwrap_or_else(|| "*".to_string());
    println!("已发送到 [{group}][{target}]");
    Ok(())
}

async fn run_send_json(config: Option<PathBuf>, args: JsonArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(config);
    let config = load_or_default_config(&config_path)?;
    let (runtime, _state) = connect_with_console_plugin(config, None).await?;
    let content = parse_json_value(&args.content)?;

    runtime.send_json(args.group.clone(), args.to.clone(), content)?;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let group = args
        .group
        .unwrap_or_else(|| runtime.identity().group_name.clone());
    let target = args.to.unwrap_or_else(|| "*".to_string());
    println!("已发送 JSON 到 [{group}][{target}]");
    Ok(())
}

async fn run_send_command(config: Option<PathBuf>, args: CommandArgs) -> anyhow::Result<()> {
    if args.to.is_none() {
        anyhow::bail!("command 仅支持单播，请使用 --to 指定目标客户端");
    }

    let config_path = resolve_config_path(config);
    let config = load_or_default_config(&config_path)?;
    let (mut runtime, _state) = connect_with_console_plugin(config, None).await?;

    runtime.send_command(
        args.group.clone(),
        args.to.clone(),
        args.command.clone(),
        Some(args.timeout),
    )?;

    let group = args
        .group
        .unwrap_or_else(|| runtime.identity().group_name.clone());
    let target = args.to.unwrap_or_else(|| "*".to_string());
    println!("已发送命令到 [{group}][{target}]");

    wait_for_command_result(&mut runtime, args.timeout).await?;
    Ok(())
}

async fn run_send_file(config: Option<PathBuf>, args: FileArgs) -> anyhow::Result<()> {
    if args.to.is_none() {
        anyhow::bail!("file 仅支持单播，请使用 --to 指定目标客户端");
    }

    let config_path = resolve_config_path(config);
    let config = load_or_default_config(&config_path)?;
    let (mut runtime, _state) = connect_with_console_plugin(config, None).await?;

    send_file_flow(
        &mut runtime,
        &args.path,
        args.group.clone(),
        args.to.clone(),
        args.timeout,
    )
    .await
}

async fn run_chat(args: ChatArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(args.config);
    let config = load_or_default_config(&config_path)?;
    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
    spawn_stdin_reader(line_tx);
    let mut session_group = args.group;
    let mut session_client = args.to;
    let mut console_state: Option<Arc<ConsoleState>> = None;
    let mut first_connection = true;

    loop {
        let (mut runtime, state) = connect_with_console_plugin_with_retry(
            &config,
            console_state.clone(),
            args.reconnect_interval_sec,
        )
        .await?;
        console_state = Some(state.clone());

        runtime.session_mut().group_name = session_group.clone();
        runtime.session_mut().client_name = session_client.clone();

        if first_connection {
            print_chat_status(&runtime);
            print_chat_help();
        } else {
            println!(
                "已重新连接 [{} / {}]",
                runtime.identity().group_name,
                runtime.identity().client_name
            );
                    state.replay_after_reconnect();
            print_chat_status(&runtime);
        }

        let heartbeat = runtime.spawn_heartbeat_loop();
        let exit = run_chat_session(&mut runtime, &state, &mut line_rx).await?;
        heartbeat.abort();

        session_group = runtime.session().group_name.clone();
        session_client = runtime.session().client_name.clone();

        match exit {
            SessionLoopExit::UserQuit => return Ok(()),
            SessionLoopExit::Disconnected if args.reconnect_interval_sec == 0 => {
                println!("连接已断开");
                return Ok(());
            }
            SessionLoopExit::Disconnected => {
                println!(
                    "连接已断开，将在 {} 秒后自动重连...",
                    args.reconnect_interval_sec.max(1)
                );
                first_connection = false;
            }
        }
    }
}

async fn run_run(args: RunArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(args.config);
    let config = load_or_default_config(&config_path)?;
    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
    spawn_stdin_reader(line_tx);
    let mut console_state: Option<Arc<ConsoleState>> = None;
    let mut first_connection = true;

    loop {
        let (mut runtime, state) = connect_with_console_plugin_with_retry(
            &config,
            console_state.clone(),
            args.reconnect_interval_sec,
        )
        .await?;
        console_state = Some(state.clone());

        if first_connection {
            println!(
                "已连接 [{} / {}]",
                runtime.identity().group_name,
                runtime.identity().client_name
            );
            print_console_control_help();
        } else {
            println!(
                "已重新连接 [{} / {}]",
                runtime.identity().group_name,
                runtime.identity().client_name
            );
                    state.replay_after_reconnect();
        }

        let heartbeat = runtime.spawn_heartbeat_loop();
        let exit = run_run_session(&mut runtime, &state, &mut line_rx).await?;
        heartbeat.abort();

        match exit {
            SessionLoopExit::UserQuit => return Ok(()),
            SessionLoopExit::Disconnected if args.reconnect_interval_sec == 0 => {
                println!("连接已断开");
                return Ok(());
            }
            SessionLoopExit::Disconnected => {
                println!(
                    "连接已断开，将在 {} 秒后自动重连...",
                    args.reconnect_interval_sec.max(1)
                );
                first_connection = false;
            }
        }
    }
}

async fn run_chat_session(
    runtime: &mut ClientRuntime,
    console_state: &Arc<ConsoleState>,
    line_rx: &mut mpsc::UnboundedReceiver<String>,
) -> anyhow::Result<SessionLoopExit> {
    loop {
        tokio::select! {
            maybe_line = line_rx.recv() => {
                match maybe_line {
                    Some(line) => {
                        if !handle_chat_line(runtime, console_state, line).await? {
                            return Ok(SessionLoopExit::UserQuit);
                        }
                    }
                    None => return Ok(SessionLoopExit::UserQuit),
                }
            }
            maybe_message = runtime.recv_next_message() => {
                match maybe_message {
                    Some(IncomingRuntimeMessage::Packet(packet)) => {
                        if let IncomingServerPacket::Ack(ack) = &packet {
                            console_state.handle_ack(ack);
                        }
                        if let IncomingServerPacket::Error(error) = &packet {
                            console_state.handle_server_error(error);
                        }
                        runtime.dispatch_packet(packet).await?;
                    }
                    Some(IncomingRuntimeMessage::Binary(frame)) => {
                        console_state.handle_binary_frame(runtime, frame)?;
                    }
                    None => return Ok(SessionLoopExit::Disconnected),
                }
            }
        }
    }
}

async fn run_run_session(
    runtime: &mut ClientRuntime,
    console_state: &Arc<ConsoleState>,
    line_rx: &mut mpsc::UnboundedReceiver<String>,
) -> anyhow::Result<SessionLoopExit> {
    let mut stdin_open = true;
    loop {
        tokio::select! {
            maybe_line = line_rx.recv(), if stdin_open => {
                match maybe_line {
                    Some(line) => handle_console_control_line(runtime, console_state, line)?,
                    None => stdin_open = false,
                }
            }
            maybe_message = runtime.recv_next_message() => {
                match maybe_message {
                    Some(IncomingRuntimeMessage::Packet(packet)) => {
                        if let IncomingServerPacket::Ack(ack) = &packet {
                            console_state.handle_ack(ack);
                        }
                        if let IncomingServerPacket::Error(error) = &packet {
                            console_state.handle_server_error(error);
                        }
                        runtime.dispatch_packet(packet).await?;
                    }
                    Some(IncomingRuntimeMessage::Binary(frame)) => {
                        console_state.handle_binary_frame(runtime, frame)?;
                    }
                    None => return Ok(SessionLoopExit::Disconnected),
                }
            }
        }
    }
}

async fn handle_chat_line(
    runtime: &mut ClientRuntime,
    console_state: &Arc<ConsoleState>,
    line: String,
) -> anyhow::Result<bool> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("/to ") {
        runtime.session_mut().client_name = Some(rest.trim().to_string());
        print_chat_status(runtime);
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("/group ") {
        runtime.session_mut().group_name = Some(rest.trim().to_string());
        print_chat_status(runtime);
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("/json ") {
        match parse_json_value(rest.trim()) {
            Ok(content) => {
                let group_name = runtime.session().group_name.clone();
                let client_name = runtime.session().client_name.clone();
                runtime.send_json(group_name, client_name, content)?;
            }
            Err(error) => {
                println!("JSON 格式错误: {error}");
            }
        }
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("/command ") {
        let command = rest.trim();
        if command.is_empty() {
            println!("请提供要执行的命令");
            return Ok(true);
        }

        if runtime.session().client_name.is_none() {
            println!("command 仅支持单播，请先使用 /to 指定目标用户");
            return Ok(true);
        }

        let group_name = runtime.session().group_name.clone();
        let client_name = runtime.session().client_name.clone();
        runtime.send_command(group_name, client_name, command, Some(30))?;
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("/file ") {
        let path = PathBuf::from(rest.trim());
        if runtime.session().client_name.is_none() {
            println!("file 仅支持单播，请先使用 /to 指定目标用户");
            return Ok(true);
        }

        let group_name = runtime.session().group_name.clone();
        let client_name = runtime.session().client_name.clone();
        console_state.start_outgoing_file(runtime, &path, group_name, client_name)?;
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("/accept ") {
        handle_file_offer_decision_with_state(runtime, console_state, rest.trim(), true)?;
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("/reject ") {
        handle_file_offer_decision_with_state(runtime, console_state, rest.trim(), false)?;
        return Ok(true);
    }

    if line == "/files" {
        console_state.print_file_queue();
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("/abort ") {
        handle_file_abort_with_state(runtime, console_state, rest.trim())?;
        return Ok(true);
    }

    match line {
        "/to" => {
            runtime.session_mut().client_name = None;
            print_chat_status(runtime);
            return Ok(true);
        }
        "/group" => {
            runtime.session_mut().group_name = None;
            print_chat_status(runtime);
            return Ok(true);
        }
        "/status" => {
            print_chat_status(runtime);
            return Ok(true);
        }
        "/help" => {
            print_chat_help();
            return Ok(true);
        }
        "/exit" => return Ok(false),
        _ => {}
    }

    let group_name = runtime.session().group_name.clone();
    let client_name = runtime.session().client_name.clone();
    runtime.send_text(group_name, client_name, line)?;
    Ok(true)
}

fn print_chat_status(runtime: &ClientRuntime) {
    let group = runtime
        .session()
        .group_name
        .clone()
        .unwrap_or_else(|| runtime.identity().group_name.clone());
    let target = runtime
        .session()
        .client_name
        .clone()
        .unwrap_or_else(|| "*".to_string());
    println!("当前会话: group={group}, to={target}");
}

fn print_chat_help() {
    println!("/to <client> 切换目标用户");
    println!("/to 清空目标用户并恢复广播");
    println!("/group <group> 切换目标分组");
    println!("/group 清空目标分组并恢复默认分组");
    println!("/json <json> 发送 JSON 消息");
    println!("/command <cmd> 发送命令请求");
    println!("/file <path> 发送文件");
    println!("/files 查看本地文件状态");
    println!("/accept <fileId> 接收待确认文件");
    println!("/reject <fileId> 拒绝待确认文件");
    println!("/abort <fileId> 取消发送或接收中的文件");
    println!("/status 查看当前会话");
    println!("/help 查看帮助");
    println!("/exit 退出会话");
}

fn resolve_config_path(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(ClientConfig::default_config_path)
}

fn load_or_default_config(path: &Path) -> anyhow::Result<ClientConfig> {
    if path.exists() {
        ClientConfig::load_from_path(path)
    } else {
        Ok(ClientConfig::default())
    }
}

fn parse_json_value(content: &str) -> anyhow::Result<Value> {
    Ok(serde_json::from_str(content)?)
}

async fn wait_for_command_result(
    runtime: &mut ClientRuntime,
    timeout_secs: u64,
) -> anyhow::Result<()> {
    let deadline = tokio::time::sleep(Duration::from_secs(timeout_secs.max(1) + 2));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => {
                println!("等待命令执行结果超时");
                break;
            }
            maybe_packet = runtime.recv_next_packet() => {
                let Some(packet) = maybe_packet else {
                    break;
                };

                let is_command_result = matches!(
                    &packet,
                    y2m_client_core::IncomingServerPacket::Event(event)
                        if event.payload.event_type == EventType::CommandResult
                );
                runtime.dispatch_packet(packet).await?;
                if is_command_result {
                    break;
                }
            }
        }
    }

    Ok(())
}

async fn send_file_flow(
    runtime: &mut ClientRuntime,
    path: &Path,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    timeout_secs: u64,
) -> anyhow::Result<()> {
    let target_client = target_client_name
        .clone()
        .ok_or_else(|| anyhow::anyhow!("file 仅支持单播，请指定目标客户端"))?;
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        anyhow::bail!("目标不是文件: {}", path.display());
    }

    let bytes = fs::read(path)?;
    let file_id = Uuid::new_v4();
    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.bin".to_string());
    let sha256 = sha256_hex(&bytes);
    let chunk_size: usize = 256 * 1024;
    let total_chunks = bytes.len().div_ceil(chunk_size) as u32;
    let target_group = target_group_name
        .clone()
        .unwrap_or_else(|| runtime.identity().group_name.clone());
    let offer_packet = build_file_offer_event_packet(
        runtime.identity(),
        target_group_name.clone(),
        Some(target_client.clone()),
        file_id,
        file_name.clone(),
        bytes.len() as u64,
        guess_content_type(path),
        sha256.clone(),
        chunk_size as u32,
        total_chunks.max(1),
    );

    println!(
        "开始发送文件: name={}, size={}, chunks={}, to=[{}][{}]",
        file_name,
        format_bytes(bytes.len() as u64),
        total_chunks.max(1),
        target_group,
        target_client
    );
    runtime.connection().send_json_packet(&offer_packet)?;
    println!("已发送文件申请，等待对方确认...");

    let accept_info = wait_for_file_accept(runtime, file_id, timeout_secs).await?;
    if let Some(save_path) = accept_info.save_path {
        println!("对方已接受，目标保存路径: {}", save_path);
    } else {
        println!("对方已接受，开始发送分片...");
    }

    let mut last_reported_percent = 0u64;
    let total_bytes = bytes.len() as u64;
    let effective_total_chunks = total_chunks.max(1);
    for (chunk_index, chunk) in bytes.chunks(chunk_size).enumerate() {
        let frame = BinaryChunkHeader::new(
            file_id,
            chunk_index as u32,
            effective_total_chunks,
            chunk.len() as u32,
        )
        .encode_with_payload(chunk);
        runtime.connection().send_binary(frame)?;

        let sent_bytes = ((chunk_index + 1) * chunk_size).min(bytes.len()) as u64;
        let percent = if total_bytes == 0 {
            100
        } else {
            sent_bytes.saturating_mul(100) / total_bytes
        };
        if effective_total_chunks <= 10 || percent >= last_reported_percent + 10 || percent == 100 {
            println!(
                "发送进度: {}% ({}/{}), chunk {}/{}",
                percent,
                format_bytes(sent_bytes),
                format_bytes(total_bytes),
                chunk_index + 1,
                effective_total_chunks
            );
            last_reported_percent = percent;
        }
    }
    if total_bytes == 0 {
        println!("发送进度: 100% (0 B/0 B), chunk 0/1");
    }

    let complete_packet = build_file_complete_event_packet(
        runtime.identity(),
        target_group_name.clone(),
        Some(target_client_name.unwrap()),
        file_id,
        bytes.len() as u64,
        sha256,
    );
    let complete_request_id = complete_packet.request_id.clone();
    runtime.connection().send_json_packet(&complete_packet)?;
    println!("分片发送完成，等待对方校验确认...");
    wait_for_file_ack(runtime, file_id, &complete_request_id, timeout_secs).await?;

    println!("文件发送完成: {} ({})", path.display(), format_bytes(total_bytes));
    Ok(())
}

async fn wait_for_file_accept(
    runtime: &mut ClientRuntime,
    file_id: Uuid,
    timeout_secs: u64,
) -> anyhow::Result<FileAcceptInfo> {
    let deadline = tokio::time::sleep(Duration::from_secs(timeout_secs.max(1)));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => {
                anyhow::bail!("等待文件接受超时");
            }
            maybe_packet = runtime.recv_next_packet() => {
                let Some(packet) = maybe_packet else {
                    anyhow::bail!("连接已关闭，未收到 file_accept");
                };

                match &packet {
                    IncomingServerPacket::Event(event)
                        if event.payload.event_type == EventType::FileAccept && matches_file_id(event, file_id) =>
                    {
                        break Ok(FileAcceptInfo {
                            save_path: event
                                .payload
                                .metadata
                                .get("savePath")
                                .and_then(|value| value.as_str())
                                .map(ToOwned::to_owned),
                        });
                    }
                    IncomingServerPacket::Event(event)
                        if event.payload.event_type == EventType::FileReject && matches_file_id(event, file_id) =>
                    {
                        let reason = event
                            .payload
                            .metadata
                            .get("reason")
                            .and_then(|value| value.as_str())
                            .unwrap_or("未提供原因");
                        anyhow::bail!("对方拒绝接收文件: {}", reason);
                    }
                    IncomingServerPacket::Event(event)
                        if event.payload.event_type == EventType::FileAbort && matches_file_id(event, file_id) =>
                    {
                        let reason = event
                            .payload
                            .metadata
                            .get("reason")
                            .and_then(|value| value.as_str())
                            .unwrap_or("未提供原因");
                        anyhow::bail!("对方已取消文件传输: {}", reason);
                    }
                    IncomingServerPacket::Error(packet) => {
                        anyhow::bail!("服务端拒绝文件发送: {}", packet.payload.message);
                    }
                    _ => runtime.dispatch_packet(packet).await?,
                }
            }
        }
    }
}

async fn wait_for_file_ack(
    runtime: &mut ClientRuntime,
    file_id: Uuid,
    request_id: &str,
    timeout_secs: u64,
) -> anyhow::Result<()> {
    let deadline = tokio::time::sleep(Duration::from_secs(timeout_secs.max(1)));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => {
                anyhow::bail!("等待文件确认超时");
            }
            maybe_packet = runtime.recv_next_packet() => {
                let Some(packet) = maybe_packet else {
                    anyhow::bail!("连接已关闭，未收到文件确认");
                };

                match packet {
                    IncomingServerPacket::Ack(packet) if packet.request_id == request_id => {
                        match packet.payload.status {
                            AckStatus::Ok => break,
                            AckStatus::Rejected => anyhow::bail!("对方拒绝确认文件"),
                        }
                    }
                    IncomingServerPacket::Event(event)
                        if event.payload.event_type == EventType::FileAbort && matches_file_id(&event, file_id) =>
                    {
                        let reason = event
                            .payload
                            .metadata
                            .get("reason")
                            .and_then(|value| value.as_str())
                            .unwrap_or("未提供原因");
                        anyhow::bail!("对方已取消文件传输: {}", reason);
                    }
                    IncomingServerPacket::Error(packet) => {
                        anyhow::bail!("服务端返回错误: {}", packet.payload.message);
                    }
                    other => runtime.dispatch_packet(other).await?,
                }
            }
        }
    }

    Ok(())
}

async fn connect_with_console_plugin(
    config: ClientConfig,
    existing_state: Option<Arc<ConsoleState>>,
) -> anyhow::Result<(ClientRuntime, Arc<ConsoleState>)> {
    let state = if let Some(state) = existing_state {
        state.clear_file_transfer_state();
        state
    } else {
        Arc::new(ConsoleState::new(config.download_dir.clone()))
    };
    let mut core = ClientCore::new(config);
    core.plugin_registry_mut()
        .register(Arc::new(ConsolePlugin { state: state.clone() }));
    let runtime = core.connect().await?;
    Ok((runtime, state))
}

async fn connect_with_console_plugin_with_retry(
    config: &ClientConfig,
    existing_state: Option<Arc<ConsoleState>>,
    reconnect_interval_sec: u64,
) -> anyhow::Result<(ClientRuntime, Arc<ConsoleState>)> {
    loop {
        match connect_with_console_plugin(config.clone(), existing_state.clone()).await {
            Ok(result) => return Ok(result),
            Err(error) if reconnect_interval_sec > 0 => {
                println!(
                    "连接失败，将在 {} 秒后重试: {}",
                    reconnect_interval_sec.max(1),
                    error
                );
                tokio::time::sleep(Duration::from_secs(reconnect_interval_sec.max(1))).await;
            }
            Err(error) => return Err(error),
        }
    }
}

fn spawn_stdin_reader(line_tx: mpsc::UnboundedSender<String>) {
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    if line_tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn parse_file_id(packet: &EventPacket) -> anyhow::Result<Uuid> {
    let file_id = packet
        .payload
        .metadata
        .get("fileId")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing fileId"))?;
    Ok(Uuid::parse_str(file_id)?)
}

fn matches_file_id(packet: &EventPacket, file_id: Uuid) -> bool {
    parse_file_id(packet).map(|value| value == file_id).unwrap_or(false)
}

fn ensure_unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }

    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let ext = path.extension().map(|value| value.to_string_lossy().to_string());
    let parent = path.parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));

    for index in 1.. {
        let candidate_name = match &ext {
            Some(ext) => format!("{stem}-{index}.{ext}"),
            None => format!("{stem}-{index}"),
        };
        let candidate = parent.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    path
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest
        .as_slice()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KiB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn guess_content_type(path: &Path) -> String {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "txt" | "md" | "log" => "text/plain".to_string(),
        "json" => "application/json".to_string(),
        "html" | "htm" => "text/html".to_string(),
        "png" => "image/png".to_string(),
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "gif" => "image/gif".to_string(),
        "pdf" => "application/pdf".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

fn handle_console_control_line(
    runtime: &ClientRuntime,
    console_state: &ConsoleState,
    line: String,
) -> anyhow::Result<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }

    if let Some(rest) = line.strip_prefix("/accept ") {
        handle_file_offer_decision_with_state(runtime, console_state, rest.trim(), true)?;
        return Ok(());
    }

    if let Some(rest) = line.strip_prefix("/reject ") {
        handle_file_offer_decision_with_state(runtime, console_state, rest.trim(), false)?;
        return Ok(());
    }

    if line == "/files" {
        console_state.print_file_queue();
        return Ok(());
    }

    if let Some(rest) = line.strip_prefix("/abort ") {
        handle_file_abort_with_state(runtime, console_state, rest.trim())?;
        return Ok(());
    }

    if line == "/help" {
        print_console_control_help();
    }

    Ok(())
}

fn handle_file_offer_decision_with_state(
    runtime: &ClientRuntime,
    console_state: &ConsoleState,
    file_id: &str,
    accept: bool,
) -> anyhow::Result<()> {
    let file_id = match Uuid::parse_str(file_id) {
        Ok(file_id) => file_id,
        Err(_) => {
            println!("fileId 格式错误: {}", file_id);
            return Ok(());
        }
    };
    let handled = if accept {
        console_state.accept_pending_offer(runtime, file_id)?
    } else {
        console_state.reject_pending_offer(runtime, file_id)?
    };

    if !handled {
        println!("未找到待处理文件: {}", file_id);
    }

    Ok(())
}

fn handle_file_abort_with_state(
    runtime: &ClientRuntime,
    console_state: &ConsoleState,
    file_id: &str,
) -> anyhow::Result<()> {
    let file_id = match Uuid::parse_str(file_id) {
        Ok(file_id) => file_id,
        Err(_) => {
            println!("fileId 格式错误: {}", file_id);
            return Ok(());
        }
    };

    if !console_state.abort_transfer(runtime, file_id)? {
        println!("未找到可取消的文件: {}", file_id);
    }

    Ok(())
}

fn print_console_control_help() {
    println!("可用控制命令:");
    println!("/files 查看本地文件状态");
    println!("/accept <fileId> 接收待确认文件");
    println!("/reject <fileId> 拒绝待确认文件");
    println!("/abort <fileId> 取消发送或接收中的文件");
    println!("/help 查看控制命令");
}
