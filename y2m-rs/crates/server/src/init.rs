use tokio::sync::mpsc::UnboundedSender;
use y2m_common::{
    Endpoint, InitAckPacket, InitAckPayload, InitPacket, Packet, PacketKind, PROTOCOL_VERSION,
};

use crate::{
    error::ServerError,
    session::{now_timestamp, ConnectionMessage, SessionRecord, SessionStore},
};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub heartbeat_interval_sec: u64,
    pub heartbeat_timeout_sec: u64,
    pub max_file_size: u64,
    pub max_chunk_size: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_sec: 60,
            heartbeat_timeout_sec: 150,
            max_file_size: 100 * 1024 * 1024,
            max_chunk_size: 256 * 1024,
        }
    }
}

pub async fn handle_init(
    store: &SessionStore,
    config: &ServerConfig,
    packet: InitPacket,
    remote_addr: Option<String>,
    outbound_tx: UnboundedSender<ConnectionMessage>,
) -> Result<(SessionRecord, InitAckPacket), ServerError> {
    if packet.version != PROTOCOL_VERSION {
        return Err(ServerError::UnsupportedVersion(packet.version));
    }

    if packet.kind != PacketKind::Init {
        return Err(ServerError::InvalidPacketKind);
    }

    let session = store
        .register(
            packet.payload.group_name.as_deref(),
            packet.payload.client_name.as_deref(),
            remote_addr,
            packet.payload.capabilities,
            outbound_tx,
        )
        .await?;

    let ack = build_init_ack(config, &packet.request_id, &session);
    Ok((session, ack))
}

fn build_init_ack(
    config: &ServerConfig,
    request_id: &str,
    session: &SessionRecord,
) -> InitAckPacket {
    Packet::new(
        PacketKind::InitAck,
        request_id.to_string(),
        now_timestamp(),
        Some(Endpoint::server()),
        Some(session.endpoint()),
        InitAckPayload {
            connection_id: session.connection_id,
            group_name: session.group_name.clone(),
            client_name: session.client_name.clone(),
            heartbeat_interval_sec: config.heartbeat_interval_sec,
            heartbeat_timeout_sec: config.heartbeat_timeout_sec,
            max_file_size: config.max_file_size,
            max_chunk_size: config.max_chunk_size,
        },
    )
}
