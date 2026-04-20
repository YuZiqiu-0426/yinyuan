use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::{mpsc::UnboundedSender, RwLock};
use uuid::Uuid;
use y2m_common::{CapabilitySet, DEFAULT_GROUP_NAME, Endpoint};

use crate::error::ServerError;

#[derive(Debug, Clone)]
pub enum ConnectionMessage {
    Text(String),
    Binary(Vec<u8>),
    Close,
}

#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub connection_id: Uuid,
    pub group_name: String,
    pub client_name: String,
    pub remote_addr: Option<String>,
    pub connected_at: i64,
    pub last_heartbeat_at: i64,
    pub capabilities: CapabilitySet,
    pub outbound_tx: UnboundedSender<ConnectionMessage>,
}

impl SessionRecord {
    pub fn endpoint(&self) -> Endpoint {
        Endpoint::new(self.group_name.clone(), self.client_name.clone())
    }
}

#[derive(Debug, Default)]
pub struct SessionStore {
    groups: RwLock<HashMap<String, HashMap<String, SessionRecord>>>,
    connections: RwLock<HashMap<Uuid, SessionRecord>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(
        &self,
        requested_group_name: Option<&str>,
        requested_client_name: Option<&str>,
        remote_addr: Option<String>,
        capabilities: CapabilitySet,
        outbound_tx: UnboundedSender<ConnectionMessage>,
    ) -> Result<SessionRecord, ServerError> {
        let group_name = requested_group_name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(DEFAULT_GROUP_NAME)
            .to_string();

        let client_name = requested_client_name
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let record = SessionRecord {
            connection_id: Uuid::new_v4(),
            group_name: group_name.clone(),
            client_name: client_name.clone(),
            remote_addr,
            connected_at: now_timestamp(),
            last_heartbeat_at: now_timestamp(),
            capabilities,
            outbound_tx,
        };

        let mut groups = self.groups.write().await;
        let group_entry = groups.entry(group_name.clone()).or_default();

        if group_entry.contains_key(&client_name) {
            return Err(ServerError::DuplicateClientName {
                group_name,
                client_name,
            });
        }

        group_entry.insert(client_name, record.clone());
        drop(groups);

        let mut connections = self.connections.write().await;
        connections.insert(record.connection_id, record.clone());

        Ok(record)
    }

    pub async fn resolve_unicast(
        &self,
        group_name: &str,
        client_name: &str,
    ) -> Result<SessionRecord, ServerError> {
        let groups = self.groups.read().await;
        let group = groups
            .get(group_name)
            .ok_or_else(|| ServerError::GroupNotFound {
                group_name: group_name.to_string(),
            })?;

        group
            .get(client_name)
            .cloned()
            .ok_or_else(|| ServerError::ClientNotFound {
                group_name: group_name.to_string(),
                client_name: client_name.to_string(),
            })
    }

    pub async fn resolve_broadcast(
        &self,
        group_name: &str,
        exclude_connection_id: Uuid,
    ) -> Result<Vec<SessionRecord>, ServerError> {
        let groups = self.groups.read().await;
        let group = groups
            .get(group_name)
            .ok_or_else(|| ServerError::GroupNotFound {
                group_name: group_name.to_string(),
            })?;

        Ok(group
            .values()
            .filter(|session| session.connection_id != exclude_connection_id)
            .cloned()
            .collect())
    }

    pub async fn resolve_connection(&self, connection_id: Uuid) -> Option<SessionRecord> {
        self.connections.read().await.get(&connection_id).cloned()
    }

    pub async fn touch_heartbeat(&self, connection_id: Uuid) {
        let now = now_timestamp();

        let mut connections = self.connections.write().await;
        let Some(connection) = connections.get_mut(&connection_id) else {
            return;
        };
        connection.last_heartbeat_at = now;

        let group_name = connection.group_name.clone();
        let client_name = connection.client_name.clone();
        drop(connections);

        let mut groups = self.groups.write().await;
        if let Some(group) = groups.get_mut(&group_name) {
            if let Some(session) = group.get_mut(&client_name) {
                session.last_heartbeat_at = now;
            }
        }
    }

    pub async fn remove_connection(&self, connection_id: Uuid) -> Option<SessionRecord> {
        let mut connections = self.connections.write().await;
        let record = connections.remove(&connection_id)?;
        drop(connections);

        let mut groups = self.groups.write().await;
        if let Some(group) = groups.get_mut(&record.group_name) {
            group.remove(&record.client_name);
            if group.is_empty() {
                groups.remove(&record.group_name);
            }
        }

        Some(record)
    }

    pub async fn expired_connection_ids(&self, heartbeat_timeout_sec: u64) -> Vec<Uuid> {
        let now = now_timestamp();
        let timeout_sec = heartbeat_timeout_sec.max(1) as i64;
        self.connections
            .read()
            .await
            .values()
            .filter(|session| now.saturating_sub(session.last_heartbeat_at) > timeout_sec)
            .map(|session| session.connection_id)
            .collect()
    }
}

pub fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
