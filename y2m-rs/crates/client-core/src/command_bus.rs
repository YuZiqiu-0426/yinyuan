use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use uuid::Uuid;
use y2m_common::{
    AckPacket, AckPayload, AckStatus, Endpoint, EventPacket, EventPayload, EventType,
    HeartbeatPacket, HeartbeatPayload, InitPacket, InitPayload, Packet, PacketKind,
};

use crate::{config::ClientConfig, session::ClientIdentity};

pub fn build_init_packet(config: &ClientConfig) -> InitPacket {
    Packet::new(
        PacketKind::Init,
        Uuid::new_v4().to_string(),
        now_timestamp(),
        None,
        None,
        InitPayload {
            group_name: config.group_name.clone(),
            client_name: config.client_name.clone(),
            token: config.token.clone(),
            capabilities: Default::default(),
        },
    )
}

pub fn build_heartbeat_packet(identity: &ClientIdentity) -> HeartbeatPacket {
    Packet::new(
        PacketKind::Heartbeat,
        Uuid::new_v4().to_string(),
        now_timestamp(),
        Some(Endpoint::new(
            identity.group_name.clone(),
            identity.client_name.clone(),
        )),
        Some(Endpoint::server()),
        HeartbeatPayload::default(),
    )
}

pub fn build_text_event_packet(
    identity: &ClientIdentity,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    content: impl Into<String>,
) -> EventPacket {
    build_event_packet(
        identity,
        target_group_name,
        target_client_name,
        EventType::Text,
        json!(content.into()),
        json!({
            "contentType": "text/plain"
        }),
    )
}

pub fn build_json_event_packet(
    identity: &ClientIdentity,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    content: Value,
) -> EventPacket {
    build_event_packet(
        identity,
        target_group_name,
        target_client_name,
        EventType::Json,
        content,
        json!({
            "contentType": "application/json"
        }),
    )
}

pub fn build_command_event_packet(
    identity: &ClientIdentity,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    command: impl Into<String>,
    timeout_sec: Option<u64>,
) -> EventPacket {
    build_event_packet(
        identity,
        target_group_name,
        target_client_name,
        EventType::Command,
        json!(command.into()),
        json!({
            "timeoutSec": timeout_sec.unwrap_or(30)
        }),
    )
}

pub fn build_command_result_packet(
    identity: &ClientIdentity,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    request_id: impl Into<String>,
    exit_code: i32,
    stdout: impl Into<String>,
    stderr: impl Into<String>,
    duration_ms: u64,
) -> EventPacket {
    build_event_packet(
        identity,
        target_group_name,
        target_client_name,
        EventType::CommandResult,
        Value::Null,
        json!({
            "requestId": request_id.into(),
            "exitCode": exit_code,
            "stdout": stdout.into(),
            "stderr": stderr.into(),
            "durationMs": duration_ms
        }),
    )
}

pub fn build_file_offer_event_packet(
    identity: &ClientIdentity,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    file_id: Uuid,
    file_name: impl Into<String>,
    file_size: u64,
    content_type: impl Into<String>,
    sha256: impl Into<String>,
    chunk_size: u32,
    total_chunks: u32,
) -> EventPacket {
    build_event_packet(
        identity,
        target_group_name,
        target_client_name,
        EventType::FileOffer,
        Value::Null,
        json!({
            "fileId": file_id,
            "fileName": file_name.into(),
            "fileSize": file_size,
            "contentType": content_type.into(),
            "sha256": sha256.into(),
            "chunkSize": chunk_size,
            "totalChunks": total_chunks
        }),
    )
}

pub fn build_file_accept_event_packet(
    identity: &ClientIdentity,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    file_id: Uuid,
    save_path: impl Into<String>,
) -> EventPacket {
    build_event_packet(
        identity,
        target_group_name,
        target_client_name,
        EventType::FileAccept,
        Value::Null,
        json!({
            "fileId": file_id,
            "savePath": save_path.into()
        }),
    )
}

pub fn build_file_reject_event_packet(
    identity: &ClientIdentity,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    file_id: Uuid,
    reason: impl Into<String>,
) -> EventPacket {
    build_event_packet(
        identity,
        target_group_name,
        target_client_name,
        EventType::FileReject,
        Value::Null,
        json!({
            "fileId": file_id,
            "reason": reason.into()
        }),
    )
}

pub fn build_file_abort_event_packet(
    identity: &ClientIdentity,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    file_id: Uuid,
    reason: impl Into<String>,
) -> EventPacket {
    build_event_packet(
        identity,
        target_group_name,
        target_client_name,
        EventType::FileAbort,
        Value::Null,
        json!({
            "fileId": file_id,
            "reason": reason.into()
        }),
    )
}

pub fn build_file_complete_event_packet(
    identity: &ClientIdentity,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    file_id: Uuid,
    file_size: u64,
    sha256: impl Into<String>,
) -> EventPacket {
    build_event_packet(
        identity,
        target_group_name,
        target_client_name,
        EventType::FileComplete,
        Value::Null,
        json!({
            "fileId": file_id,
            "fileSize": file_size,
            "sha256": sha256.into()
        }),
    )
}

pub fn build_ack_packet(
    identity: &ClientIdentity,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    request_id: impl Into<String>,
    ref_kind: PacketKind,
    ref_type: Option<EventType>,
    status: AckStatus,
) -> AckPacket {
    Packet::new(
        PacketKind::Ack,
        request_id.into(),
        now_timestamp(),
        Some(Endpoint::new(
            identity.group_name.clone(),
            identity.client_name.clone(),
        )),
        Some(Endpoint {
            group_name: target_group_name,
            client_name: target_client_name,
        }),
        AckPayload {
            ref_kind,
            ref_type,
            status,
        },
    )
}

pub fn build_event_packet(
    identity: &ClientIdentity,
    target_group_name: Option<String>,
    target_client_name: Option<String>,
    event_type: EventType,
    content: Value,
    metadata: Value,
) -> EventPacket {
    Packet::new(
        PacketKind::Event,
        Uuid::new_v4().to_string(),
        now_timestamp(),
        Some(Endpoint::new(
            identity.group_name.clone(),
            identity.client_name.clone(),
        )),
        Some(Endpoint {
            group_name: target_group_name,
            client_name: target_client_name,
        }),
        EventPayload {
            event_type,
            content,
            metadata,
        },
    )
}

fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
