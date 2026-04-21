use std::{fs, path::Path};

use uuid::Uuid;
use y2m_client_core::{
    build_ack_packet, build_file_abort_event_packet, build_file_accept_event_packet,
    build_file_reject_event_packet, ClientConnection, ClientIdentity, ClientRuntime, PluginContext,
};
use y2m_common::{AckPacket, AckStatus, ErrorCode, ErrorPacket, EventPacket, EventType, PacketKind};

use crate::{
    file_store::{FileTransferFailureReason, LocalFileTransfer},
    printer::cprintln,
    util::{ensure_unique_path, format_bytes, format_sender_context_line, parse_file_id, sha256_hex},
};

use super::ConsoleState;

pub(crate) struct FileOfferInfo {
    pub(crate) file_id: Uuid,
    pub(crate) file_name: String,
    pub(crate) expected_size: u64,
    pub(crate) expected_sha256: String,
    pub(crate) total_chunks: u32,
    pub(crate) source_group: String,
    pub(crate) source_client: String,
    pub(crate) save_path: std::path::PathBuf,
}

pub(crate) fn extract_file_offer_info(
    ctx: &PluginContext,
    packet: &EventPacket,
    downloads_dir: &Path,
) -> anyhow::Result<FileOfferInfo> {
    let file_id = parse_file_id(packet)?;
    let file_name = packet.payload.metadata.get("fileName")
        .and_then(|v| v.as_str()).unwrap_or("unknown.bin").to_string();
    let expected_size = packet.payload.metadata.get("fileSize")
        .and_then(|v| v.as_u64()).unwrap_or(0);
    let expected_sha256 = packet.payload.metadata.get("sha256")
        .and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let total_chunks = packet.payload.metadata.get("totalChunks")
        .and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    let source_group = packet.source.as_ref()
        .and_then(|e| e.group_name.clone())
        .unwrap_or_else(|| ctx.identity.group_name.clone());
    let source_client = packet.source.as_ref()
        .and_then(|e| e.client_name.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let save_path = ensure_unique_path(downloads_dir.join(&file_name));
    Ok(FileOfferInfo {
        file_id, file_name, expected_size, expected_sha256,
        total_chunks, source_group, source_client, save_path,
    })
}

fn print_file_offer_notice(info: &FileOfferInfo) {
    cprintln!(
        "收到文件请求: id={}, from=[{}][{}], name={}, size={} bytes, 保存到={}",
        info.file_id, info.source_group, info.source_client,
        info.file_name, info.expected_size, info.save_path.display()
    );
}

impl ConsoleState {
    pub(crate) fn handle_file_offer(&self, ctx: &PluginContext, packet: &EventPacket) -> anyhow::Result<()> {
        let info = extract_file_offer_info(ctx, packet, &self.downloads_dir)?;
        print_file_offer_notice(&info);
        if let Some(ctx) = format_sender_context_line(&packet.payload.metadata) {
            cprintln!("  发送方终端: {ctx}");
        }
        self.insert_pending_offer(
            info.file_id,
            LocalFileTransfer::pending_offer(
                info.source_group.clone(), info.source_client.clone(),
                info.file_name.clone(), info.save_path.clone(),
                info.expected_size, info.expected_sha256, info.total_chunks,
            ),
        );
        if self.download_dir_configured {
            self.send_accept_for_pending(&ctx.identity, &ctx.connection, info.file_id)?;
            cprintln!("（已在配置中指定下载目录，无需手动 /accept）");
        } else {
            cprintln!("输入 /accept {} 接收，或 /reject {} 拒绝", info.file_id, info.file_id);
            cprintln!("也可以输入 /files 查看当前文件列表");
        }
        Ok(())
    }

    pub(crate) fn accept_pending_offer(&self, runtime: &ClientRuntime, file_id: Uuid) -> anyhow::Result<bool> {
        self.send_accept_for_pending(runtime.identity(), runtime.connection(), file_id)
    }

    pub(crate) fn send_accept_for_pending(
        &self,
        identity: &ClientIdentity,
        connection: &ClientConnection,
        file_id: Uuid,
    ) -> anyhow::Result<bool> {
        let Some(offer) = self.move_pending_offer_to_incoming(file_id) else { return Ok(false) };
        let save_path = offer.save_path.as_ref().expect("pending offer save path");
        let accept = build_file_accept_event_packet(
            identity, Some(offer.peer_group), Some(offer.peer_client),
            file_id, save_path.display().to_string(),
        );
        connection.send_json_packet(&accept)?;
        cprintln!("已接受文件: id={}, 保存到={}, 等待分片传输...", file_id, save_path.display());
        Ok(true)
    }

    pub(crate) fn reject_pending_offer(&self, runtime: &ClientRuntime, file_id: Uuid) -> anyhow::Result<bool> {
        let Some(offer) = self.take_pending_offer(file_id) else { return Ok(false) };
        let reject = build_file_reject_event_packet(
            runtime.identity(), Some(offer.peer_group), Some(offer.peer_client),
            file_id, "rejected by user",
        );
        runtime.connection().send_json_packet(&reject)?;
        cprintln!("已拒绝文件: id={}, name={}", file_id, offer.file_name);
        Ok(true)
    }

    pub(crate) fn fail_incoming_transfer(
        &self, runtime: &ClientRuntime, file_id: Uuid, reason: FileTransferFailureReason,
    ) -> anyhow::Result<()> {
        let Some(pending) = self.take_incoming_file(file_id) else { return Ok(()) };
        let reason_text = reason.reason_text();
        cprintln!("文件接收失败: id={}, name={}, reason={}", file_id, pending.file_name, reason_text);
        let abort = build_file_abort_event_packet(
            runtime.identity(), Some(pending.peer_group), Some(pending.peer_client), file_id, &reason_text,
        );
        runtime.connection().send_json_packet(&abort)?;
        Ok(())
    }

    pub(crate) fn handle_binary_frame(&self, runtime: &ClientRuntime, frame: Vec<u8>) -> anyhow::Result<()> {
        let (header, payload) = y2m_common::BinaryChunkHeader::decode(&frame)
            .ok_or_else(|| anyhow::anyhow!("invalid binary chunk"))?;
        match self.mutate_incoming_file(header.file_id, |t| t.apply_chunk(&header, payload)) {
            Some(Ok(Some((file_name, received, total_bytes)))) => {
                let percent = if total_bytes == 0 { 100 } else { received.saturating_mul(100) / total_bytes };
                cprintln!("接收进度: file={}, {}% ({}/{}), chunk {}/{}",
                    file_name, percent, format_bytes(received), format_bytes(total_bytes),
                    header.chunk_index + 1, header.total_chunks);
            }
            Some(Ok(None)) => {}
            Some(Err(reason)) => self.fail_incoming_transfer(runtime, header.file_id, reason)?,
            None => {}
        }
        Ok(())
    }

    pub(crate) fn handle_file_complete(&self, ctx: &PluginContext, packet: &EventPacket) -> anyhow::Result<()> {
        let file_id = parse_file_id(packet)?;
        let Some(pending) = self.take_incoming_file(file_id) else { return Ok(()) };
        let status = compute_file_complete_status(&pending);
        if status == AckStatus::Ok {
            let save_path = pending.save_path.as_ref().expect("incoming save path");
            if let Some(parent) = save_path.parent() { fs::create_dir_all(parent)?; }
            fs::write(save_path, &pending.incoming_bytes)?;
            cprintln!("文件已保存: {}", save_path.display());
        }
        let ack = build_ack_packet(
            &ctx.identity, Some(pending.peer_group), Some(pending.peer_client),
            packet.request_id.clone(), PacketKind::Event, Some(EventType::FileComplete), status,
        );
        ctx.connection.send_json_packet(&ack)?;
        Ok(())
    }

    pub(crate) fn handle_file_abort(&self, packet: &EventPacket) -> anyhow::Result<()> {
        let file_id = parse_file_id(packet)?;
        let reason = packet.payload.metadata.get("reason").and_then(|v| v.as_str()).unwrap_or("未提供原因");
        if let Some(t) = self.take_outgoing_file(file_id) {
            cprintln!("发送已取消: id={}, name={}, reason={}", file_id, t.file_name, reason);
        }
        self.take_pending_offer(file_id);
        if let Some(t) = self.take_incoming_file(file_id) {
            cprintln!("接收已取消: id={}, name={}, reason={}", file_id, t.file_name, reason);
        }
        Ok(())
    }

    pub(crate) fn handle_file_reject(&self, packet: &EventPacket) -> anyhow::Result<()> {
        let file_id = parse_file_id(packet)?;
        let reason = packet.payload.metadata.get("reason").and_then(|v| v.as_str()).unwrap_or("未提供原因");
        if let Some(t) = self.take_outgoing_file(file_id) {
            cprintln!("对方拒绝接收文件: id={}, name={}, reason={}", file_id, t.file_name, reason);
        }
        self.take_pending_offer(file_id);
        self.take_incoming_file(file_id);
        Ok(())
    }

    pub(crate) fn abort_transfer(&self, runtime: &ClientRuntime, file_id: Uuid) -> anyhow::Result<bool> {
        if let Some(t) = self.take_outgoing_file(file_id) {
            send_abort(runtime, &t.peer_group, &t.peer_client, file_id, "aborted by user")?;
            cprintln!("已取消发送: id={}, name={}", file_id, t.file_name);
            return Ok(true);
        }
        if let Some(t) = self.take_incoming_file(file_id) {
            send_abort(runtime, &t.peer_group, &t.peer_client, file_id, "aborted by user")?;
            cprintln!("已取消接收: id={}, name={}", file_id, t.file_name);
            return Ok(true);
        }
        if let Some(t) = self.take_pending_offer(file_id) {
            send_abort(runtime, &t.peer_group, &t.peer_client, file_id, "aborted by user")?;
            cprintln!("已取消待确认文件: id={}, name={}", file_id, t.file_name);
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn handle_ack(&self, packet: &AckPacket) {
        if packet.payload.ref_type != Some(EventType::FileComplete) { return; }
        let Some((file_id, transfer)) = self.take_outgoing_by_completion_request_id(&packet.request_id) else { return };
        match packet.payload.status {
            AckStatus::Ok => cprintln!(
                "文件发送完成: {} ({})",
                transfer.source_path.as_ref().expect("outgoing source path").display(),
                transfer.file_name
            ),
            AckStatus::Rejected => cprintln!("对方拒绝确认文件: id={}, name={}", file_id, transfer.file_name),
        }
    }

    pub(crate) fn handle_server_error(&self, packet: &ErrorPacket) {
        let maybe_file_id = packet.payload.details.get("fileId")
            .and_then(|v| v.as_str()).and_then(|v| Uuid::parse_str(v).ok());
        if let Some(file_id) = maybe_file_id {
            if let Some(t) = self.take_outgoing_file(file_id) {
                cprintln!("文件发送失败: id={}, name={}, code={:?}, message={}",
                    file_id, t.file_name, packet.payload.code, packet.payload.message);
                return;
            }
        }
        if matches!(packet.payload.code,
            ErrorCode::FileTooLarge | ErrorCode::FileTransferNotAccepted | ErrorCode::InvalidMessage) {
            cprintln!("服务端错误: code={:?}, message={}", packet.payload.code, packet.payload.message);
        }
    }
}

fn compute_file_complete_status(pending: &LocalFileTransfer) -> AckStatus {
    if pending.next_chunk_index != pending.total_chunks {
        let reason = FileTransferFailureReason::MissingChunks {
            expected_chunks: pending.total_chunks,
            received_chunks: pending.next_chunk_index,
        };
        cprintln!("文件接收失败: name={}, reason={}", pending.file_name, reason.reason_text());
        return AckStatus::Rejected;
    }
    let actual_sha256 = sha256_hex(&pending.incoming_bytes);
    if pending.incoming_bytes.len() as u64 == pending.bytes_total
        && (pending.sha256.is_empty() || actual_sha256 == pending.sha256)
    {
        AckStatus::Ok
    } else {
        cprintln!(
            "文件校验失败: name={}, expected_size={}, actual_size={}, expected_sha256={}, actual_sha256={}",
            pending.file_name, pending.bytes_total, pending.incoming_bytes.len(), pending.sha256, actual_sha256
        );
        AckStatus::Rejected
    }
}

fn send_abort(runtime: &ClientRuntime, peer_group: &str, peer_client: &str, file_id: Uuid, reason: &str) -> anyhow::Result<()> {
    let packet = build_file_abort_event_packet(
        runtime.identity(), Some(peer_group.to_string()), Some(peer_client.to_string()), file_id, reason,
    );
    runtime.connection().send_json_packet(&packet)
}
