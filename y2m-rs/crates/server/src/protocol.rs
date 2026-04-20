use serde::{Deserialize, Serialize};
use serde_json::Value;
use y2m_common::{
    AckPacket, ErrorPacket, EventPacket, HeartbeatPacket, InitPacket, PacketKind,
};

use crate::ServerError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPacket {
    kind: PacketKind,
}

#[derive(Debug)]
pub enum IncomingTextPacket {
    Init(InitPacket),
    Heartbeat(HeartbeatPacket),
    Event(EventPacket),
    Ack(AckPacket),
    Error(ErrorPacket),
}

pub fn decode_text_packet(text: &str) -> Result<IncomingTextPacket, ServerError> {
    let raw: RawPacket = serde_json::from_str(text)
        .map_err(|_| ServerError::InvalidPacketKind)?;

    let value: Value = serde_json::from_str(text)
        .map_err(|_| ServerError::InvalidPacketKind)?;

    match raw.kind {
        PacketKind::Init => serde_json::from_value(value)
            .map(IncomingTextPacket::Init)
            .map_err(|_| ServerError::InvalidPacketKind),
        PacketKind::Heartbeat => serde_json::from_value(value)
            .map(IncomingTextPacket::Heartbeat)
            .map_err(|_| ServerError::InvalidPacketKind),
        PacketKind::Event => serde_json::from_value(value)
            .map(IncomingTextPacket::Event)
            .map_err(|_| ServerError::InvalidPacketKind),
        PacketKind::Ack => serde_json::from_value(value)
            .map(IncomingTextPacket::Ack)
            .map_err(|_| ServerError::InvalidPacketKind),
        PacketKind::Error => serde_json::from_value(value)
            .map(IncomingTextPacket::Error)
            .map_err(|_| ServerError::InvalidPacketKind),
        PacketKind::InitAck | PacketKind::HeartbeatAck => Err(ServerError::InvalidPacketKind),
    }
}

pub fn encode_packet<T: Serialize>(packet: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string(packet)?)
}
