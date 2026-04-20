use serde::Deserialize;
use serde_json::Value;
use y2m_common::{AckPacket, ErrorPacket, EventPacket, HeartbeatAckPacket, InitAckPacket, PacketKind};

use crate::ClientCoreError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPacket {
    kind: PacketKind,
}

#[derive(Debug)]
pub enum IncomingServerPacket {
    InitAck(InitAckPacket),
    HeartbeatAck(HeartbeatAckPacket),
    Event(EventPacket),
    Ack(AckPacket),
    Error(ErrorPacket),
}

pub fn decode_server_packet(text: &str) -> Result<IncomingServerPacket, ClientCoreError> {
    let raw: RawPacket =
        serde_json::from_str(text).map_err(|_| ClientCoreError::UnexpectedPacketKind)?;
    let value: Value =
        serde_json::from_str(text).map_err(|_| ClientCoreError::UnexpectedPacketKind)?;

    match raw.kind {
        PacketKind::InitAck => serde_json::from_value(value)
            .map(IncomingServerPacket::InitAck)
            .map_err(|_| ClientCoreError::UnexpectedPacketKind),
        PacketKind::HeartbeatAck => serde_json::from_value(value)
            .map(IncomingServerPacket::HeartbeatAck)
            .map_err(|_| ClientCoreError::UnexpectedPacketKind),
        PacketKind::Event => serde_json::from_value(value)
            .map(IncomingServerPacket::Event)
            .map_err(|_| ClientCoreError::UnexpectedPacketKind),
        PacketKind::Ack => serde_json::from_value(value)
            .map(IncomingServerPacket::Ack)
            .map_err(|_| ClientCoreError::UnexpectedPacketKind),
        PacketKind::Error => serde_json::from_value(value)
            .map(IncomingServerPacket::Error)
            .map_err(|_| ClientCoreError::UnexpectedPacketKind),
        PacketKind::Init | PacketKind::Heartbeat => Err(ClientCoreError::UnexpectedPacketKind),
    }
}
