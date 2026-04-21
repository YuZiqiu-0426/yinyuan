use std::{path::Path, time::Duration};

use uuid::Uuid;
use y2m_client_core::{
    build_file_complete_event_packet, build_file_offer_event_packet, ClientRuntime,
    IncomingServerPacket,
};
use y2m_common::{AckStatus, BinaryChunkHeader, EventType};

use crate::{
    printer::cprintln,
    types::FileAcceptInfo,
    util::{format_bytes, guess_content_type, matches_file_id, sha256_hex},
};

pub(crate) async fn send_file_flow(
    runtime: &mut ClientRuntime,
    path: &Path,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    timeout_secs: u64,
) -> anyhow::Result<()> {
    let broadcast = target_client_name.is_none();
    let bytes = std::fs::read(path)?;
    if !std::fs::metadata(path)?.is_file() { anyhow::bail!("目标不是文件: {}", path.display()); }
    let file_id = Uuid::new_v4();
    let file_name = path.file_name().map(|v| v.to_string_lossy().to_string()).unwrap_or_else(|| "unknown.bin".to_string());
    let sha256 = sha256_hex(&bytes);
    let chunk_size: usize = 256 * 1024;
    let total_chunks = bytes.len().div_ceil(chunk_size) as u32;
    let effective_chunks = total_chunks.max(1);
    let target_group = target_group_name.clone().unwrap_or_else(|| runtime.identity().group_name.clone());
    let offer = build_file_offer_event_packet(
        runtime.identity(),
        target_group_name.clone(),
        target_client_name.clone(),
        file_id,
        file_name.clone(),
        bytes.len() as u64,
        guess_content_type(path),
        sha256.clone(),
        chunk_size as u32,
        effective_chunks,
    );
    let to_disp = target_client_name.as_deref().unwrap_or("*");
    cprintln!(
        "开始发送文件: name={}, size={}, chunks={}, to=[{}][{}]",
        file_name,
        format_bytes(bytes.len() as u64),
        effective_chunks,
        target_group,
        to_disp
    );
    runtime.connection().send_json_packet(&offer)?;
    cprintln!("已发送文件申请，等待对方确认...");
    let accept_info =
        wait_for_file_accept(runtime, file_id, timeout_secs, broadcast).await?;
    if let Some(sp) = accept_info.save_path { cprintln!("对方已接受，目标保存路径: {}", sp); }
    else { cprintln!("对方已接受，开始发送分片..."); }
    send_chunks_with_progress(runtime, &bytes, file_id, chunk_size, effective_chunks)?;
    let complete = build_file_complete_event_packet(
        runtime.identity(),
        target_group_name,
        target_client_name,
        file_id,
        bytes.len() as u64,
        sha256,
    );
    let complete_request_id = complete.request_id.clone();
    runtime.connection().send_json_packet(&complete)?;
    cprintln!("分片发送完成，等待对方校验确认...");
    wait_for_file_ack(runtime, file_id, &complete_request_id, timeout_secs).await?;
    cprintln!("文件发送完成: {} ({})", path.display(), format_bytes(bytes.len() as u64));
    Ok(())
}

fn send_chunks_with_progress(
    runtime: &ClientRuntime,
    bytes: &[u8],
    file_id: Uuid,
    chunk_size: usize,
    total_chunks: u32,
) -> anyhow::Result<()> {
    let total_bytes = bytes.len() as u64;
    let mut last_reported_percent = 0u64;
    for (chunk_index, chunk) in bytes.chunks(chunk_size).enumerate() {
        let frame = BinaryChunkHeader::new(file_id, chunk_index as u32, total_chunks, chunk.len() as u32)
            .encode_with_payload(chunk);
        runtime.connection().send_binary(frame)?;
        let sent_bytes = ((chunk_index + 1) * chunk_size).min(bytes.len()) as u64;
        let percent = if total_bytes == 0 { 100 } else { sent_bytes.saturating_mul(100) / total_bytes };
        if total_chunks <= 10 || percent >= last_reported_percent + 10 || percent == 100 {
            cprintln!("发送进度: {}% ({}/{}), chunk {}/{}",
                percent, format_bytes(sent_bytes), format_bytes(total_bytes), chunk_index + 1, total_chunks);
            last_reported_percent = percent;
        }
    }
    if total_bytes == 0 { cprintln!("发送进度: 100% (0 B/0 B), chunk 0/1"); }
    Ok(())
}

pub(crate) async fn wait_for_file_accept(
    runtime: &mut ClientRuntime,
    file_id: Uuid,
    timeout_secs: u64,
    broadcast_offer: bool,
) -> anyhow::Result<FileAcceptInfo> {
    let deadline = tokio::time::sleep(Duration::from_secs(timeout_secs.max(1)));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => anyhow::bail!("等待文件接受超时"),
            maybe_packet = runtime.recv_next_packet() => {
                let Some(packet) = maybe_packet else { anyhow::bail!("连接已关闭，未收到 file_accept") };
                match packet {
                    IncomingServerPacket::Event(event)
                        if event.payload.event_type == EventType::FileAccept && matches_file_id(&event, file_id) =>
                    {
                        let info = FileAcceptInfo {
                            save_path: event.payload.metadata.get("savePath")
                                .and_then(|v| v.as_str()).map(ToOwned::to_owned),
                        };
                        if broadcast_offer {
                            // 给同组其他成员一点时间完成 file_accept，便于服务端登记多条传输腿。
                            tokio::time::sleep(Duration::from_millis(280)).await;
                        }
                        return Ok(info);
                    }
                    IncomingServerPacket::Event(event)
                        if event.payload.event_type == EventType::FileReject && matches_file_id(&event, file_id) =>
                    {
                        let reason = event.payload.metadata.get("reason")
                            .and_then(|v| v.as_str()).unwrap_or("未提供原因");
                        anyhow::bail!("对方拒绝接收文件: {}", reason);
                    }
                    IncomingServerPacket::Event(event)
                        if event.payload.event_type == EventType::FileAbort && matches_file_id(&event, file_id) =>
                    {
                        let reason = event.payload.metadata.get("reason")
                            .and_then(|v| v.as_str()).unwrap_or("未提供原因");
                        anyhow::bail!("对方已取消文件传输: {}", reason);
                    }
                    IncomingServerPacket::Error(p) => anyhow::bail!("服务端拒绝文件发送: {}", p.payload.message),
                    other => runtime.dispatch_packet(other).await?,
                }
            }
        }
    }
}

pub(crate) async fn wait_for_file_ack(
    runtime: &mut ClientRuntime,
    file_id: Uuid,
    request_id: &str,
    timeout_secs: u64,
) -> anyhow::Result<()> {
    let deadline = tokio::time::sleep(Duration::from_secs(timeout_secs.max(1)));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => anyhow::bail!("等待文件确认超时"),
            maybe_packet = runtime.recv_next_packet() => {
                let Some(packet) = maybe_packet else { anyhow::bail!("连接已关闭，未收到文件确认") };
                match packet {
                    IncomingServerPacket::Ack(p) if p.request_id == request_id => {
                        match p.payload.status {
                            AckStatus::Ok => break,
                            AckStatus::Rejected => anyhow::bail!("对方拒绝确认文件"),
                        }
                    }
                    IncomingServerPacket::Event(event)
                        if event.payload.event_type == EventType::FileAbort && matches_file_id(&event, file_id) =>
                    {
                        let reason = event.payload.metadata.get("reason")
                            .and_then(|v| v.as_str()).unwrap_or("未提供原因");
                        anyhow::bail!("对方已取消文件传输: {}", reason);
                    }
                    IncomingServerPacket::Error(p) => anyhow::bail!("服务端返回错误: {}", p.payload.message),
                    other => runtime.dispatch_packet(other).await?,
                }
            }
        }
    }
    Ok(())
}
