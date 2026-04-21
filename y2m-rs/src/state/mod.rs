use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Mutex,
};

use uuid::Uuid;

use crate::{
    file_store::{
        FileTransferFailureReason, LocalFileEntry, LocalFileState, LocalFileStore,
        LocalFileTransfer, LocalTransferView,
    },
    printer::cprintln,
};

mod command;
mod file_handling;
mod outgoing;


#[derive(Default)]
pub(crate) struct ConsoleState {
    pub(crate) downloads_dir: PathBuf,
    /// Set when config explicitly provides `download_dir` (`y2m init --download-dir` / JSON). Incoming offers auto-accept.
    pub(crate) download_dir_configured: bool,
    pub(crate) files: Mutex<LocalFileStore>,
    pub(crate) reconnect_replays: Mutex<Vec<String>>,
}

impl ConsoleState {
    pub(crate) fn new(downloads_dir: Option<PathBuf>) -> Self {
        let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let download_dir_configured = downloads_dir.is_some();
        let downloads_dir = downloads_dir.unwrap_or_else(|| base.join("downloads"));
        Self {
            downloads_dir,
            download_dir_configured,
            files: Mutex::new(LocalFileStore::default()),
            reconnect_replays: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn transfer_failure_line(
        view: LocalTransferView,
        file_id: Uuid,
        file_name: &str,
        reason: &FileTransferFailureReason,
    ) -> String {
        let reason = reason.reason_text();
        match view {
            LocalTransferView::PendingOffer =>
                format!("待确认文件已失效: id={}, name={}, reason={}", file_id, file_name, reason),
            LocalTransferView::Incoming =>
                format!("接收失败: id={}, name={}, reason={}", file_id, file_name, reason),
            LocalTransferView::Outgoing =>
                format!("发送失败: id={}, name={}, reason={}", file_id, file_name, reason),
        }
    }

    pub(crate) fn queue_reconnect_failure(
        &self,
        view: LocalTransferView,
        file_id: Uuid,
        file_name: &str,
        reason: FileTransferFailureReason,
    ) {
        self.reconnect_replays
            .lock()
            .expect("lock reconnect replays")
            .push(Self::transfer_failure_line(view, file_id, file_name, &reason));
    }

    pub(crate) fn drain_local_file_entries_for_reconnect(&self) -> Vec<LocalFileEntry> {
        let transfers = {
            let mut files = self.files.lock().expect("lock local file store");
            std::mem::take(&mut files.transfers).into_iter().collect::<Vec<_>>()
        };
        transfers.iter().map(|(file_id, t)| Self::local_entry_from_transfer(*file_id, t)).collect()
    }

    pub(crate) fn local_entry_from_transfer(file_id: Uuid, transfer: &LocalFileTransfer) -> LocalFileEntry {
        LocalFileEntry {
            file_id,
            view: transfer.view,
            state: transfer.state,
            peer_group: transfer.peer_group.clone(),
            peer_client: transfer.peer_client.clone(),
            file_name: transfer.file_name.clone(),
            bytes_done: transfer.bytes_done,
            bytes_total: transfer.bytes_total,
            save_path: transfer.save_path.clone(),
        }
    }

    pub(crate) fn insert_pending_offer(&self, file_id: Uuid, transfer: LocalFileTransfer) {
        self.files.lock().expect("lock local file store").transfers.insert(file_id, transfer);
    }

    pub(crate) fn take_pending_offer(&self, file_id: Uuid) -> Option<LocalFileTransfer> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.remove(&file_id)?;
        if transfer.ensure_pending_offer("take pending offer").is_ok() {
            Some(transfer)
        } else {
            files.transfers.insert(file_id, transfer);
            None
        }
    }

    pub(crate) fn take_incoming_file(&self, file_id: Uuid) -> Option<LocalFileTransfer> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.remove(&file_id)?;
        if transfer.ensure_incoming("take incoming transfer").is_ok() {
            Some(transfer)
        } else {
            files.transfers.insert(file_id, transfer);
            None
        }
    }

    pub(crate) fn insert_outgoing_file(&self, file_id: Uuid, transfer: LocalFileTransfer) {
        self.files.lock().expect("lock local file store").transfers.insert(file_id, transfer);
    }

    pub(crate) fn take_outgoing_file(&self, file_id: Uuid) -> Option<LocalFileTransfer> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.remove(&file_id)?;
        if transfer.ensure_outgoing("take outgoing transfer").is_ok() {
            Some(transfer)
        } else {
            files.transfers.insert(file_id, transfer);
            None
        }
    }

    pub(crate) fn has_outgoing_file(&self, file_id: Uuid) -> bool {
        self.files.lock().expect("lock local file store")
            .transfers.get(&file_id).is_some_and(LocalFileTransfer::is_outgoing)
    }

    pub(crate) fn take_outgoing_by_completion_request_id(&self, request_id: &str) -> Option<(Uuid, LocalFileTransfer)> {
        let mut files = self.files.lock().expect("lock local file store");
        let file_id = find_waiting_ack_by_request_id(&files.transfers, request_id)?;
        let transfer = files.transfers.remove(&file_id)?;
        if transfer.ensure_waiting_ack("take outgoing transfer by completion request id").is_ok() {
            Some((file_id, transfer))
        } else {
            files.transfers.insert(file_id, transfer);
            None
        }
    }

    pub(crate) fn mutate_incoming_file<R>(&self, file_id: Uuid, f: impl FnOnce(&mut LocalFileTransfer) -> R) -> Option<R> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.get_mut(&file_id)?;
        if transfer.ensure_incoming("mutate incoming transfer").is_ok() {
            Some(f(transfer))
        } else {
            None
        }
    }

    pub(crate) fn mutate_outgoing_file<R>(&self, file_id: Uuid, f: impl FnOnce(&mut LocalFileTransfer) -> R) -> Option<R> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.get_mut(&file_id)?;
        if transfer.ensure_outgoing("mutate outgoing transfer").is_ok() {
            Some(f(transfer))
        } else {
            None
        }
    }

    pub(crate) fn move_pending_offer_to_incoming(&self, file_id: Uuid) -> Option<LocalFileTransfer> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.get_mut(&file_id)?;
        if transfer.move_to_incoming().is_err() {
            return None;
        }
        Some(transfer.clone())
    }

    pub(crate) fn start_outgoing_waiting_accept(&self, file_id: Uuid, transfer: LocalFileTransfer) {
        self.insert_outgoing_file(file_id, transfer);
    }

    pub(crate) fn update_outgoing_phase(
        &self,
        file_id: Uuid,
        expected_from: LocalFileState,
        phase: LocalFileState,
        completion_request_id: Option<String>,
    ) -> Option<LocalFileTransfer> {
        let mut files = self.files.lock().expect("lock local file store");
        let transfer = files.transfers.get_mut(&file_id)?;
        if transfer.transition_to(expected_from, phase, completion_request_id).is_err() {
            return None;
        }
        Some(transfer.clone())
    }

    pub(crate) fn clear_file_transfer_state(&self) {
        for entry in self.drain_local_file_entries_for_reconnect() {
            self.queue_reconnect_failure(
                entry.view, entry.file_id, &entry.file_name,
                FileTransferFailureReason::Disconnected,
            );
        }
    }

    pub(crate) fn replay_after_reconnect(&self) {
        let messages = {
            let mut replays = self.reconnect_replays.lock().expect("lock reconnect replays");
            std::mem::take(&mut *replays)
        };
        for message in messages {
            cprintln!("{message}");
        }
    }

    pub(crate) fn snapshot_local_file_entries(&self) -> Vec<LocalFileEntry> {
        let files = self.files.lock().expect("lock local file store");
        let mut entries: Vec<LocalFileEntry> = files.transfers.iter()
            .map(|(file_id, t)| Self::local_entry_from_transfer(*file_id, t))
            .collect();
        entries.sort_by(|l, r| l.file_id.cmp(&r.file_id));
        entries
    }
}

fn find_waiting_ack_by_request_id(transfers: &HashMap<Uuid, LocalFileTransfer>, request_id: &str) -> Option<Uuid> {
    transfers.iter().find_map(|(file_id, transfer)| {
        (transfer.completion_request_id.as_deref() == Some(request_id)
            && transfer.ensure_waiting_ack("find by completion request id").is_ok())
            .then_some(*file_id)
    })
}
