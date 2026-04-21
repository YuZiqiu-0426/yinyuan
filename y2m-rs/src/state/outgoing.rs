use std::{fs, path::Path, sync::Arc};

use uuid::Uuid;
use y2m_client_core::{
    build_file_complete_event_packet, build_file_offer_event_packet, ClientConnection,
    ClientIdentity, ClientRuntime, PluginContext,
};
use y2m_common::{BinaryChunkHeader, EventPacket};

use crate::{
    file_store::{LocalFileState, LocalFileTransfer, LocalTransferView},
    printer::cprintln,
    util::{format_bytes, guess_content_type, parse_file_id, sha256_hex},
};

use super::ConsoleState;

struct OutgoingFileMeta {
    file_id: Uuid,
    file_name: String,
    file_size: u64,
    chunk_size: usize,
    total_chunks: u32,
    sha256: String,
    bytes: Arc<Vec<u8>>,
    target_group: String,
    /// `None` means group broadcast (display as `*`).
    target_client: Option<String>,
    content_type: String,
}

fn prepare_outgoing_file(
    path: &Path,
    runtime: &ClientRuntime,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
) -> anyhow::Result<OutgoingFileMeta> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() { anyhow::bail!("目标不是文件: {}", path.display()); }
    let bytes = Arc::new(fs::read(path)?);
    let file_id = Uuid::new_v4();
    let file_name = path.file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.bin".to_string());
    let file_size = bytes.len() as u64;
    let chunk_size = 256 * 1024usize;
    let total_chunks = bytes.len().div_ceil(chunk_size) as u32;
    let target_group = target_group_name.unwrap_or_else(|| runtime.identity().group_name.clone());
    let content_type = guess_content_type(path);
    let sha256 = sha256_hex(bytes.as_slice());
    Ok(OutgoingFileMeta {
        file_id,
        file_name,
        file_size,
        chunk_size,
        total_chunks: total_chunks.max(1),
        sha256,
        bytes,
        target_group,
        target_client: target_client_name,
        content_type,
    })
}

impl ConsoleState {
    pub(crate) fn start_outgoing_file(
        self: &Arc<Self>,
        runtime: &ClientRuntime,
        path: &Path,
        target_group_name: Option<String>,
        target_client_name: Option<String>,
    ) -> anyhow::Result<()> {
        let meta = prepare_outgoing_file(path, runtime, target_group_name, target_client_name)?;
        let peer_label = meta
            .target_client
            .clone()
            .unwrap_or_else(|| "*".to_string());
        self.start_outgoing_waiting_accept(
            meta.file_id,
            LocalFileTransfer::outgoing(
                meta.target_group.clone(),
                peer_label,
                meta.file_name.clone(),
                path.to_path_buf(),
                meta.file_size,
                meta.total_chunks,
                meta.chunk_size,
                meta.sha256.clone(),
                meta.bytes.clone(),
            ),
        );
        let offer = build_file_offer_event_packet(
            runtime.identity(),
            Some(meta.target_group.clone()),
            meta.target_client.clone(),
            meta.file_id,
            meta.file_name.clone(),
            meta.file_size,
            meta.content_type,
            meta.sha256,
            meta.chunk_size as u32,
            meta.total_chunks,
        );
        let to_client = meta.target_client.as_deref().unwrap_or("*");
        cprintln!(
            "开始发送文件: id={}, name={}, size={}, chunks={}, to=[{}][{}]",
            meta.file_id,
            meta.file_name,
            format_bytes(meta.file_size),
            meta.total_chunks,
            meta.target_group,
            to_client
        );
        runtime.connection().send_json_packet(&offer)?;
        cprintln!("已发送文件申请，等待对方确认...");
        Ok(())
    }

    pub(crate) async fn handle_outgoing_file_accept(
        self: &Arc<Self>,
        ctx: &PluginContext,
        packet: &EventPacket,
    ) -> anyhow::Result<()> {
        let file_id = parse_file_id(packet)?;
        let save_path = packet.payload.metadata.get("savePath")
            .and_then(|v| v.as_str()).map(ToOwned::to_owned);
        let Some(transfer) = self.update_outgoing_phase(
            file_id, LocalFileState::WaitingAccept, LocalFileState::Sending, None,
        ) else { return Ok(()) };
        if let Some(sp) = save_path { cprintln!("对方已接受，目标保存路径: {}", sp); }
        else { cprintln!("对方已接受，开始发送分片..."); }
        spawn_file_send_task(self.clone(), ctx, file_id, transfer);
        Ok(())
    }

    pub(crate) async fn run_outgoing_file_send(
        self: Arc<Self>,
        connection: ClientConnection,
        identity: ClientIdentity,
        file_id: Uuid,
        target_group: String,
        target_client: String,
        file_name: String,
        file_size: u64,
        total_chunks: u32,
        chunk_size: usize,
        sha256: String,
        bytes: Arc<Vec<u8>>,
    ) -> anyhow::Result<()> {
        if !send_chunks_loop(&self, &connection, &bytes, file_id, total_chunks, chunk_size, file_size).await? {
            return Ok(());
        }
        if file_size == 0 { cprintln!("发送进度: 100% (0 B/0 B), chunk 0/1"); }
        let complete = build_file_complete_event_packet(
            &identity,
            Some(target_group),
            (!target_client.is_empty() && target_client != "*").then_some(target_client.clone()),
            file_id,
            file_size,
            sha256,
        );
        let complete_request_id = complete.request_id.clone();
        connection.send_json_packet(&complete)?;
        if self.update_outgoing_phase(file_id, LocalFileState::Sending, LocalFileState::WaitingAck, Some(complete_request_id)).is_none() {
            return Ok(());
        }
        cprintln!("分片发送完成，等待对方校验确认... id={}, file={}", file_id, file_name);
        Ok(())
    }

    pub(crate) fn update_outgoing_progress(&self, file_id: Uuid, total_bytes: u64, chunk_index: usize, chunk_size: usize) {
        let update = self.mutate_outgoing_file(file_id, |transfer| {
            let sent_bytes = (chunk_index * chunk_size).min(
                transfer.outgoing_bytes.as_ref().expect("outgoing bytes").len()
            ) as u64;
            transfer.bytes_done = sent_bytes;
            let percent = if total_bytes == 0 { 100 } else { sent_bytes.saturating_mul(100) / total_bytes };
            let should_report = transfer.total_chunks <= 10
                || percent >= transfer.last_reported_percent + 10 || percent == 100;
            if should_report { transfer.last_reported_percent = percent; Some((sent_bytes, transfer.total_chunks, percent)) }
            else { None }
        });
        if let Some(Some((sent_bytes, total_chunks, percent))) = update {
            cprintln!("发送进度: {}% ({}/{}), chunk {}/{}",
                percent, format_bytes(sent_bytes), format_bytes(total_bytes), chunk_index, total_chunks);
        }
    }

    pub(crate) fn print_file_queue(&self) {
        let entries = self.snapshot_local_file_entries();
        if entries.is_empty() { cprintln!("当前没有待处理文件"); return; }
        let pending: Vec<_> = entries.iter().filter(|e| matches!(e.view, LocalTransferView::PendingOffer)).collect();
        if !pending.is_empty() {
            cprintln!("待确认文件:");
            for e in pending {
                cprintln!("  id={}, 状态={}, from=[{}][{}], name={}, size={} bytes, 保存到={}",
                    e.file_id, e.state.label(), e.peer_group, e.peer_client, e.file_name, e.bytes_total,
                    e.save_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".to_string()));
            }
        }
        let incoming: Vec<_> = entries.iter().filter(|e| matches!(e.view, LocalTransferView::Incoming)).collect();
        if !incoming.is_empty() {
            cprintln!("接收中文件:");
            for e in incoming {
                cprintln!("  id={}, 状态={}, from=[{}][{}], name={}, 进度={}/{} bytes, 保存到={}",
                    e.file_id, e.state.label(), e.peer_group, e.peer_client, e.file_name,
                    e.bytes_done, e.bytes_total,
                    e.save_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".to_string()));
            }
        }
        let outgoing: Vec<_> = entries.iter().filter(|e| matches!(e.view, LocalTransferView::Outgoing)).collect();
        if !outgoing.is_empty() {
            cprintln!("外发文件:");
            for e in outgoing {
                cprintln!("  id={}, 状态={}, to=[{}][{}], name={}, 进度={}/{} bytes",
                    e.file_id, e.state.label(), e.peer_group, e.peer_client,
                    e.file_name, e.bytes_done, e.bytes_total);
            }
        }
    }
}

fn spawn_file_send_task(state: Arc<ConsoleState>, ctx: &PluginContext, file_id: Uuid, transfer: LocalFileTransfer) {
    let connection = ctx.connection.clone();
    let identity = ctx.identity.clone();
    let target_group = transfer.peer_group.clone();
    let target_client = transfer.peer_client.clone();
    let file_name = transfer.file_name.clone();
    let file_size = transfer.bytes_total;
    let total_chunks = transfer.total_chunks;
    let chunk_size = transfer.chunk_size.expect("outgoing chunk size");
    let sha256 = transfer.sha256.clone();
    let bytes = transfer.outgoing_bytes.expect("outgoing bytes");
    tokio::spawn(async move {
        if let Err(e) = state.clone().run_outgoing_file_send(
            connection, identity, file_id, target_group, target_client,
            file_name, file_size, total_chunks, chunk_size, sha256, bytes,
        ).await {
            cprintln!("文件发送失败: {}", e);
            state.take_outgoing_file(file_id);
        }
    });
}

async fn send_chunks_loop(
    state: &ConsoleState,
    connection: &ClientConnection,
    bytes: &[u8],
    file_id: Uuid,
    total_chunks: u32,
    chunk_size: usize,
    file_size: u64,
) -> anyhow::Result<bool> {
    for (chunk_index, chunk) in bytes.chunks(chunk_size).enumerate() {
        if !state.has_outgoing_file(file_id) { return Ok(false); }
        let frame = BinaryChunkHeader::new(file_id, chunk_index as u32, total_chunks, chunk.len() as u32)
            .encode_with_payload(chunk);
        connection.send_binary(frame)?;
        state.update_outgoing_progress(file_id, file_size, chunk_index + 1, chunk_size);
        tokio::task::yield_now().await;
    }
    if !state.has_outgoing_file(file_id) { return Ok(false); }
    Ok(true)
}
