use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ClientIdentity {
    pub connection_id: Uuid,
    pub group_name: String,
    pub client_name: String,
    pub heartbeat_interval_sec: u64,
    pub heartbeat_timeout_sec: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ChatSession {
    pub group_name: Option<String>,
    pub client_name: Option<String>,
}

impl ChatSession {
    pub fn resolved_group_name(&self, identity: &ClientIdentity) -> String {
        self.group_name
            .clone()
            .unwrap_or_else(|| identity.group_name.clone())
    }
}
