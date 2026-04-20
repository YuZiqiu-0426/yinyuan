use std::collections::HashMap;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::ServerError;

#[derive(Debug, Clone)]
pub struct TransferRecord {
    pub file_id: Uuid,
    pub request_id: String,
    pub sender_connection_id: Uuid,
    pub receiver_connection_id: Uuid,
    pub accepted: bool,
}

#[derive(Debug, Default)]
pub struct TransferRegistry {
    by_file_id: RwLock<HashMap<Uuid, TransferRecord>>,
    by_request_id: RwLock<HashMap<String, Uuid>>,
}

impl TransferRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create_offer(
        &self,
        file_id: Uuid,
        request_id: String,
        sender_connection_id: Uuid,
        receiver_connection_id: Uuid,
    ) {
        let record = TransferRecord {
            file_id,
            request_id: request_id.clone(),
            sender_connection_id,
            receiver_connection_id,
            accepted: false,
        };

        self.by_file_id.write().await.insert(file_id, record);
        self.by_request_id.write().await.insert(request_id, file_id);
    }

    pub async fn mark_accepted(
        &self,
        file_id: Uuid,
        receiver_connection_id: Uuid,
    ) -> Result<TransferRecord, ServerError> {
        let mut transfers = self.by_file_id.write().await;
        let record = transfers
            .get_mut(&file_id)
            .ok_or(ServerError::FileTransferNotAccepted)?;

        if record.receiver_connection_id != receiver_connection_id {
            return Err(ServerError::FileTransferNotAccepted);
        }

        record.accepted = true;
        Ok(record.clone())
    }

    pub async fn get_by_file_id(&self, file_id: Uuid) -> Option<TransferRecord> {
        self.by_file_id.read().await.get(&file_id).cloned()
    }

    pub async fn update_request_id(
        &self,
        file_id: Uuid,
        request_id: String,
    ) -> Result<TransferRecord, ServerError> {
        let mut by_file_id = self.by_file_id.write().await;
        let record = by_file_id
            .get_mut(&file_id)
            .ok_or(ServerError::FileTransferNotAccepted)?;
        let old_request_id = record.request_id.clone();
        record.request_id = request_id.clone();
        let updated = record.clone();
        drop(by_file_id);

        let mut by_request_id = self.by_request_id.write().await;
        by_request_id.remove(&old_request_id);
        by_request_id.insert(request_id, file_id);

        Ok(updated)
    }

    pub async fn remove_by_file_id(&self, file_id: Uuid) -> Option<TransferRecord> {
        let mut by_file_id = self.by_file_id.write().await;
        let record = by_file_id.remove(&file_id)?;
        drop(by_file_id);

        self.by_request_id.write().await.remove(&record.request_id);
        Some(record)
    }

    pub async fn remove_by_request_id(&self, request_id: &str) -> Option<TransferRecord> {
        let mut by_request_id = self.by_request_id.write().await;
        let file_id = by_request_id.remove(request_id)?;
        drop(by_request_id);

        self.by_file_id.write().await.remove(&file_id)
    }

    pub async fn remove_by_connection(&self, connection_id: Uuid) -> Vec<TransferRecord> {
        let mut by_file_id = self.by_file_id.write().await;
        let removed: Vec<TransferRecord> = by_file_id
            .values()
            .filter(|record| {
                record.sender_connection_id == connection_id
                    || record.receiver_connection_id == connection_id
            })
            .cloned()
            .collect();

        for record in &removed {
            by_file_id.remove(&record.file_id);
        }
        drop(by_file_id);

        let mut by_request_id = self.by_request_id.write().await;
        for record in &removed {
            by_request_id.remove(&record.request_id);
        }

        removed
    }
}
