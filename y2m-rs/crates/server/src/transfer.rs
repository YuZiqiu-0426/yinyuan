use std::collections::{HashMap, HashSet};

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

type LegKey = (Uuid, Uuid);

#[derive(Debug, Default)]
pub struct TransferRegistry {
    legs: RwLock<HashMap<LegKey, TransferRecord>>,
    /// Maps `file_complete` (and similar) ack `request_id` -> `file_id` for cleanup after ack.
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
        let key = (file_id, receiver_connection_id);
        let record = TransferRecord {
            file_id,
            request_id: request_id.clone(),
            sender_connection_id,
            receiver_connection_id,
            accepted: false,
        };
        self.legs.write().await.insert(key, record);
    }

    pub async fn mark_accepted(
        &self,
        file_id: Uuid,
        receiver_connection_id: Uuid,
    ) -> Result<TransferRecord, ServerError> {
        let key = (file_id, receiver_connection_id);
        let mut legs = self.legs.write().await;
        let record = legs
            .get_mut(&key)
            .ok_or(ServerError::FileTransferNotAccepted)?;
        record.accepted = true;
        Ok(record.clone())
    }

    pub async fn accepted_sender_legs(
        &self,
        file_id: Uuid,
        sender_connection_id: Uuid,
    ) -> Vec<TransferRecord> {
        self.legs
            .read()
            .await
            .values()
            .filter(|r| {
                r.file_id == file_id
                    && r.sender_connection_id == sender_connection_id
                    && r.accepted
            })
            .cloned()
            .collect()
    }

    /// After `file_complete`, all legs for `file_id` share the same `request_id` (e.g. broadcast).
    pub async fn set_complete_request_id_for_file(
        &self,
        file_id: Uuid,
        new_request_id: String,
    ) -> Result<(), ServerError> {
        let mut legs = self.legs.write().await;
        let keys: Vec<LegKey> = legs
            .keys()
            .filter(|(fid, _)| *fid == file_id)
            .copied()
            .collect();
        if keys.is_empty() {
            return Err(ServerError::FileTransferNotAccepted);
        }
        let mut old_ids = HashSet::new();
        for key in &keys {
            if let Some(r) = legs.get(key) {
                old_ids.insert(r.request_id.clone());
            }
        }
        for key in keys {
            if let Some(r) = legs.get_mut(&key) {
                r.request_id = new_request_id.clone();
            }
        }
        drop(legs);

        let mut by_request_id = self.by_request_id.write().await;
        for oid in old_ids {
            by_request_id.remove(&oid);
        }
        by_request_id.insert(new_request_id, file_id);
        Ok(())
    }

    pub async fn remove_by_file_id(&self, file_id: Uuid) {
        let removed = {
            let mut legs = self.legs.write().await;
            let keys: Vec<LegKey> = legs
                .keys()
                .filter(|(fid, _)| *fid == file_id)
                .copied()
                .collect();
            let mut out = Vec::new();
            for key in keys {
                if let Some(rec) = legs.remove(&key) {
                    out.push(rec);
                }
            }
            out
        };
        let mut by_request_id = self.by_request_id.write().await;
        for rec in removed {
            by_request_id.remove(&rec.request_id);
        }
    }

    /// Removes all legs for the file associated with this complete/ack `request_id` (idempotent).
    pub async fn remove_by_request_id(&self, request_id: &str) {
        let Some(file_id) = self.by_request_id.write().await.remove(request_id) else {
            return;
        };
        let mut legs = self.legs.write().await;
        let keys: Vec<LegKey> = legs
            .keys()
            .filter(|(fid, _)| *fid == file_id)
            .copied()
            .collect();
        for key in keys {
            legs.remove(&key);
        }
    }

    pub async fn remove_by_connection(&self, connection_id: Uuid) -> Vec<TransferRecord> {
        let keys: Vec<LegKey> = {
            let legs = self.legs.read().await;
            legs
                .iter()
                .filter(|(_, r)| {
                    r.sender_connection_id == connection_id
                        || r.receiver_connection_id == connection_id
                })
                .map(|(k, _)| *k)
                .collect()
        };

        let mut removed = Vec::new();
        let mut legs = self.legs.write().await;
        for key in keys {
            if let Some(rec) = legs.remove(&key) {
                removed.push(rec);
            }
        }
        drop(legs);

        let mut by_request_id = self.by_request_id.write().await;
        for rec in &removed {
            by_request_id.remove(&rec.request_id);
        }

        removed
    }
}
