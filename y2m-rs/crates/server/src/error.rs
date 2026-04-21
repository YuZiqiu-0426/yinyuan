use serde_json::{Map, Value};
use thiserror::Error;
use y2m_common::{Endpoint, ErrorCode, ErrorPacket, ErrorPayload, Packet, PacketKind};

use crate::session::now_timestamp;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(String),
    #[error("invalid packet kind for handler")]
    InvalidPacketKind,
    #[error("client `{client_name}` already exists in group `{group_name}`")]
    DuplicateClientName {
        group_name: String,
        client_name: String,
    },
    #[error("target client `{client_name}` not found in group `{group_name}`")]
    ClientNotFound {
        group_name: String,
        client_name: String,
    },
    #[error("target group `{group_name}` not found")]
    GroupNotFound { group_name: String },
    #[error("file offer exceeds limit: {field_name}={actual}, max={max}")]
    FileTooLarge {
        file_id: Option<String>,
        file_name: Option<String>,
        field_name: String,
        actual: u64,
        max: u64,
    },
    #[error("heartbeat timeout")]
    HeartbeatTimeout,
    #[error("file transfer not accepted")]
    FileTransferNotAccepted,
    #[error("invalid binary chunk frame")]
    InvalidBinaryChunk,
}

impl ServerError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::UnsupportedVersion(_) => ErrorCode::UnsupportedVersion,
            Self::InvalidPacketKind => ErrorCode::InvalidMessage,
            Self::DuplicateClientName { .. } => ErrorCode::DuplicateClientName,
            Self::ClientNotFound { .. } => ErrorCode::ClientNotFound,
            Self::GroupNotFound { .. } => ErrorCode::GroupNotFound,
            Self::FileTooLarge { .. } => ErrorCode::FileTooLarge,
            Self::HeartbeatTimeout => ErrorCode::HeartbeatTimeout,
            Self::FileTransferNotAccepted => ErrorCode::FileTransferNotAccepted,
            Self::InvalidBinaryChunk => ErrorCode::InvalidMessage,
        }
    }

    pub fn retriable(&self) -> bool {
        matches!(self, Self::ClientNotFound { .. } | Self::GroupNotFound { .. })
    }

    pub fn to_packet(
        &self,
        request_id: impl Into<String>,
        target: Option<Endpoint>,
    ) -> ErrorPacket {
        let details = match self {
            Self::FileTooLarge {
                file_id,
                file_name,
                field_name,
                actual,
                max,
            } => {
                let mut details = Map::new();
                details.insert("fieldName".to_string(), Value::String(field_name.clone()));
                details.insert("actual".to_string(), Value::from(*actual));
                details.insert("max".to_string(), Value::from(*max));
                if let Some(file_id) = file_id {
                    details.insert("fileId".to_string(), Value::String(file_id.clone()));
                }
                if let Some(file_name) = file_name {
                    details.insert("fileName".to_string(), Value::String(file_name.clone()));
                }
                Value::Object(details)
            }
            _ => Value::Object(Map::new()),
        };
        Packet::new(
            PacketKind::Error,
            request_id,
            now_timestamp(),
            Some(Endpoint::server()),
            target,
            ErrorPayload {
                code: self.code(),
                message: self.to_string(),
                retriable: self.retriable(),
                details,
            },
        )
    }
}
