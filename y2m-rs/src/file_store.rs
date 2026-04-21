use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
};

use uuid::Uuid;
use y2m_common::BinaryChunkHeader;

#[derive(Default)]
pub(crate) struct LocalFileStore {
    pub(crate) transfers: HashMap<Uuid, LocalFileTransfer>,
}

#[derive(Clone)]
pub(crate) struct LocalFileTransfer {
    pub(crate) view: LocalTransferView,
    pub(crate) state: LocalFileState,
    pub(crate) peer_group: String,
    pub(crate) peer_client: String,
    pub(crate) file_name: String,
    pub(crate) bytes_total: u64,
    pub(crate) total_chunks: u32,
    pub(crate) sha256: String,
    pub(crate) save_path: Option<PathBuf>,
    pub(crate) source_path: Option<PathBuf>,
    pub(crate) chunk_size: Option<usize>,
    pub(crate) outgoing_bytes: Option<Arc<Vec<u8>>>,
    pub(crate) incoming_bytes: Vec<u8>,
    pub(crate) next_chunk_index: u32,
    pub(crate) bytes_done: u64,
    pub(crate) last_reported_percent: u64,
    pub(crate) completion_request_id: Option<String>,
}

impl LocalFileTransfer {
    pub(crate) fn pending_offer(
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn outgoing(
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

    pub(crate) fn move_to_incoming(&mut self) -> Result<(), LocalFileTransitionError> {
        self.ensure_pending_offer("move pending offer to incoming")?;
        self.view = LocalTransferView::Incoming;
        self.state = LocalFileState::Receiving;
        self.incoming_bytes.clear();
        self.next_chunk_index = 0;
        self.bytes_done = 0;
        self.last_reported_percent = 0;
        Ok(())
    }

    pub(crate) fn is_pending_offer(&self) -> bool {
        matches!(self.state, LocalFileState::PendingOffer)
    }

    pub(crate) fn is_incoming(&self) -> bool {
        matches!(self.state, LocalFileState::Receiving)
    }

    pub(crate) fn is_outgoing(&self) -> bool {
        matches!(
            self.state,
            LocalFileState::WaitingAccept | LocalFileState::Sending | LocalFileState::WaitingAck
        )
    }

    pub(crate) fn is_waiting_ack(&self) -> bool {
        matches!(self.state, LocalFileState::WaitingAck)
    }

    pub(crate) fn ensure_pending_offer(&self, action: &'static str) -> Result<(), LocalFileTransitionError> {
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

    pub(crate) fn ensure_incoming(&self, action: &'static str) -> Result<(), LocalFileTransitionError> {
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

    pub(crate) fn ensure_outgoing(&self, action: &'static str) -> Result<(), LocalFileTransitionError> {
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

    pub(crate) fn ensure_waiting_ack(&self, action: &'static str) -> Result<(), LocalFileTransitionError> {
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

    pub(crate) fn transition_to(
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

    pub(crate) fn apply_chunk(
        &mut self,
        header: &BinaryChunkHeader,
        payload: &[u8],
    ) -> Result<Option<(String, u64, u64)>, FileTransferFailureReason> {
        validate_incoming_chunk(header, self)?;
        self.incoming_bytes.extend_from_slice(payload);
        self.next_chunk_index += 1;
        self.bytes_done = self.incoming_bytes.len() as u64;
        if self.bytes_done > self.bytes_total {
            return Err(FileTransferFailureReason::Protocol(format!(
                "received bytes exceed expected size: expected {}, got {}",
                self.bytes_total, self.bytes_done
            )));
        }
        if should_report_progress(self, header) {
            Ok(Some((self.file_name.clone(), self.bytes_done, self.bytes_total)))
        } else {
            Ok(None)
        }
    }
}

fn validate_incoming_chunk(header: &BinaryChunkHeader, pending: &LocalFileTransfer) -> Result<(), FileTransferFailureReason> {
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
    Ok(())
}

fn should_report_progress(pending: &mut LocalFileTransfer, header: &BinaryChunkHeader) -> bool {
    let total_bytes = pending.bytes_total;
    let received_bytes = pending.bytes_done;
    let percent = if total_bytes == 0 { 100 } else { received_bytes.saturating_mul(100) / total_bytes };
    let should = pending.total_chunks <= 10
        || percent >= pending.last_reported_percent + 10
        || percent == 100
        || header.chunk_index + 1 == header.total_chunks;
    if should {
        pending.last_reported_percent = percent;
    }
    should
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalFileTransitionError {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalTransferView {
    PendingOffer,
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalFileState {
    PendingOffer,
    Receiving,
    WaitingAccept,
    Sending,
    WaitingAck,
}

impl LocalFileState {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::PendingOffer => "待确认",
            Self::Receiving => "接收中",
            Self::WaitingAccept => "等待对方接受",
            Self::Sending => "发送中",
            Self::WaitingAck => "等待对方确认",
        }
    }
}

pub(crate) struct LocalFileEntry {
    pub(crate) file_id: Uuid,
    pub(crate) view: LocalTransferView,
    pub(crate) state: LocalFileState,
    pub(crate) peer_group: String,
    pub(crate) peer_client: String,
    pub(crate) file_name: String,
    pub(crate) bytes_done: u64,
    pub(crate) bytes_total: u64,
    pub(crate) save_path: Option<PathBuf>,
}

pub(crate) enum FileTransferFailureReason {
    Disconnected,
    Protocol(String),
    MissingChunks { expected_chunks: u32, received_chunks: u32 },
}

impl FileTransferFailureReason {
    pub(crate) fn reason_text(&self) -> String {
        match self {
            Self::Disconnected => "连接中断，请等待对方在线后重新发送".to_string(),
            Self::Protocol(reason) => reason.clone(),
            Self::MissingChunks { expected_chunks, received_chunks } => format!(
                "分片不完整，expected_chunks={}, received_chunks={}",
                expected_chunks, received_chunks
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pending_transfer() -> LocalFileTransfer {
        LocalFileTransfer::pending_offer(
            "g".to_string(), "c".to_string(), "a.txt".to_string(),
            PathBuf::from("downloads/a.txt"), 12, "sha".to_string(), 1,
        )
    }

    fn make_outgoing_transfer() -> LocalFileTransfer {
        LocalFileTransfer::outgoing(
            "g".to_string(), "c".to_string(), "a.txt".to_string(),
            PathBuf::from("a.txt"), 12, 1, 1024, "sha".to_string(),
            Arc::new(vec![1, 2, 3]),
        )
    }

    #[test]
    fn move_to_incoming_returns_explicit_error_for_wrong_state() {
        let mut transfer = make_outgoing_transfer();
        let error = transfer.move_to_incoming().expect_err("should reject wrong state");
        assert_eq!(error, LocalFileTransitionError::UnexpectedState {
            action: "move pending offer to incoming",
            expected: "pending offer",
            actual: LocalFileState::WaitingAccept,
        });
    }

    #[test]
    fn transition_to_returns_explicit_error_for_invalid_from_state() {
        let mut transfer = make_outgoing_transfer();
        transfer.state = LocalFileState::WaitingAccept;
        let error = transfer
            .transition_to(LocalFileState::Sending, LocalFileState::WaitingAck, None)
            .expect_err("should reject invalid transition");
        assert_eq!(error, LocalFileTransitionError::InvalidTransition {
            expected_from: LocalFileState::Sending,
            actual: LocalFileState::WaitingAccept,
            to: LocalFileState::WaitingAck,
        });
    }

    #[test]
    fn move_to_incoming_clears_progress_and_switches_state() {
        let mut transfer = make_pending_transfer();
        transfer.bytes_done = 9;
        transfer.last_reported_percent = 70;
        transfer.move_to_incoming().expect("pending offer should move to incoming");
        assert_eq!(transfer.view, LocalTransferView::Incoming);
        assert_eq!(transfer.state, LocalFileState::Receiving);
        assert_eq!(transfer.bytes_done, 0);
        assert_eq!(transfer.last_reported_percent, 0);
    }
}
