use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

pub const PROTOCOL_VERSION: &str = "v3";
pub const DEFAULT_GROUP_NAME: &str = "default";
pub const SYSTEM_GROUP_NAME: &str = "system";
pub const SERVER_CLIENT_NAME: &str = "server";
pub const BINARY_MAGIC: [u8; 4] = *b"Y2MB";
pub const BINARY_FRAME_TYPE_FILE_CHUNK: u8 = 1;

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn null_value() -> Value {
    Value::Null
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub group_name: Option<String>,
    pub client_name: Option<String>,
}

impl Endpoint {
    pub fn new(group_name: impl Into<String>, client_name: impl Into<String>) -> Self {
        Self {
            group_name: Some(group_name.into()),
            client_name: Some(client_name.into()),
        }
    }

    pub fn server() -> Self {
        Self::new(SYSTEM_GROUP_NAME, SERVER_CLIENT_NAME)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PacketKind {
    Init,
    InitAck,
    Heartbeat,
    HeartbeatAck,
    Event,
    Ack,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Text,
    Command,
    CommandResult,
    Json,
    FileOffer,
    FileAccept,
    FileReject,
    FileComplete,
    FileAbort,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AckStatus {
    Ok,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidMessage,
    UnsupportedVersion,
    DuplicateClientName,
    ClientNotFound,
    GroupNotFound,
    FileTooLarge,
    FileTransferNotAccepted,
    Unauthorized,
    HeartbeatTimeout,
    InternalError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Packet<T> {
    pub version: String,
    pub kind: PacketKind,
    pub request_id: String,
    pub timestamp: i64,
    pub source: Option<Endpoint>,
    pub target: Option<Endpoint>,
    pub payload: T,
}

impl<T> Packet<T> {
    pub fn new(
        kind: PacketKind,
        request_id: impl Into<String>,
        timestamp: i64,
        source: Option<Endpoint>,
        target: Option<Endpoint>,
        payload: T,
    ) -> Self {
        Self {
            version: PROTOCOL_VERSION.to_string(),
            kind,
            request_id: request_id.into(),
            timestamp,
            source,
            target,
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySet {
    #[serde(default)]
    pub file_transfer_v3: bool,
    #[serde(default)]
    pub command_plugin: bool,
    #[serde(default)]
    pub json_plugin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitPayload {
    pub group_name: Option<String>,
    pub client_name: Option<String>,
    pub token: Option<String>,
    #[serde(default)]
    pub capabilities: CapabilitySet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitAckPayload {
    pub connection_id: Uuid,
    pub group_name: String,
    pub client_name: String,
    pub heartbeat_interval_sec: u64,
    pub heartbeat_timeout_sec: u64,
    pub max_file_size: u64,
    pub max_chunk_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HeartbeatPayload {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPayload {
    #[serde(rename = "type")]
    pub event_type: EventType,
    #[serde(default = "null_value")]
    pub content: Value,
    #[serde(default = "empty_object")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AckPayload {
    pub ref_kind: PacketKind,
    pub ref_type: Option<EventType>,
    pub status: AckStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub code: ErrorCode,
    pub message: String,
    pub retriable: bool,
    #[serde(default = "empty_object")]
    pub details: Value,
}

pub type InitPacket = Packet<InitPayload>;
pub type InitAckPacket = Packet<InitAckPayload>;
pub type HeartbeatPacket = Packet<HeartbeatPayload>;
pub type HeartbeatAckPacket = Packet<HeartbeatPayload>;
pub type EventPacket = Packet<EventPayload>;
pub type AckPacket = Packet<AckPayload>;
pub type ErrorPacket = Packet<ErrorPayload>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryChunkHeader {
    pub version: u8,
    pub frame_type: u8,
    pub file_id: Uuid,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub payload_size: u32,
}

impl BinaryChunkHeader {
    pub const ENCODED_LEN: usize = 36;

    pub fn new(file_id: Uuid, chunk_index: u32, total_chunks: u32, payload_size: u32) -> Self {
        Self {
            version: 3,
            frame_type: BINARY_FRAME_TYPE_FILE_CHUNK,
            file_id,
            chunk_index,
            total_chunks,
            payload_size,
        }
    }

    pub fn encode_with_payload(&self, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::ENCODED_LEN + payload.len());
        bytes.extend_from_slice(&BINARY_MAGIC);
        bytes.push(self.version);
        bytes.push(self.frame_type);
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(self.file_id.as_bytes());
        bytes.extend_from_slice(&self.chunk_index.to_le_bytes());
        bytes.extend_from_slice(&self.total_chunks.to_le_bytes());
        bytes.extend_from_slice(&self.payload_size.to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    pub fn decode(frame: &[u8]) -> Option<(Self, &[u8])> {
        if frame.len() < Self::ENCODED_LEN {
            return None;
        }

        if frame[0..4] != BINARY_MAGIC {
            return None;
        }

        let version = frame[4];
        let frame_type = frame[5];
        let file_id = Uuid::from_slice(&frame[8..24]).ok()?;
        let chunk_index = u32::from_le_bytes(frame[24..28].try_into().ok()?);
        let total_chunks = u32::from_le_bytes(frame[28..32].try_into().ok()?);
        let payload_size = u32::from_le_bytes(frame[32..36].try_into().ok()?);
        let payload = &frame[36..];

        if payload.len() != payload_size as usize {
            return None;
        }

        Some((
            Self {
                version,
                frame_type,
                file_id,
                chunk_index,
                total_chunks,
                payload_size,
            },
            payload,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformKind {
    Windows,
    Unix,
}

pub fn current_platform() -> PlatformKind {
    if cfg!(windows) {
        PlatformKind::Windows
    } else {
        PlatformKind::Unix
    }
}

pub fn default_shell_program() -> &'static str {
    match current_platform() {
        PlatformKind::Windows => "cmd",
        PlatformKind::Unix => "sh",
    }
}

pub fn default_shell_arg() -> &'static str {
    match current_platform() {
        PlatformKind::Windows => "/C",
        PlatformKind::Unix => "-c",
    }
}
