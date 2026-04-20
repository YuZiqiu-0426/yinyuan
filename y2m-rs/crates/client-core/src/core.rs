use std::time::Duration;

use serde_json::Value;
use tokio::{sync::mpsc, task::JoinHandle, time};
use tracing::warn;
use y2m_common::ErrorPacket;

use crate::{
    build_command_event_packet, build_heartbeat_packet, build_init_packet, build_json_event_packet,
    build_text_event_packet, config::ClientConfig, connection::ClientConnection,
    error::ClientCoreError, plugin::{PluginContext, PluginRegistry},
    protocol::IncomingServerPacket, session::{ChatSession, ClientIdentity},
};

pub struct ClientCore {
    config: ClientConfig,
    plugins: PluginRegistry,
}

impl ClientCore {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            plugins: PluginRegistry::new(),
        }
    }

    pub fn plugin_registry_mut(&mut self) -> &mut PluginRegistry {
        &mut self.plugins
    }

    pub async fn connect(self) -> anyhow::Result<ClientRuntime> {
        let (connection, mut inbound_rx, binary_rx) =
            ClientConnection::connect(&self.config.server_url).await?;

        let init_packet = build_init_packet(&self.config);
        connection.send_json_packet(&init_packet)?;

        let init_ack = wait_for_init_ack(&mut inbound_rx).await?;
        let heartbeat_interval_sec = self
            .config
            .heartbeat_interval_override_sec
            .unwrap_or(init_ack.payload.heartbeat_interval_sec);

        let identity = ClientIdentity {
            connection_id: init_ack.payload.connection_id,
            group_name: init_ack.payload.group_name,
            client_name: init_ack.payload.client_name,
            heartbeat_interval_sec,
            heartbeat_timeout_sec: init_ack.payload.heartbeat_timeout_sec,
        };

        Ok(ClientRuntime {
            connection,
            identity,
            inbound_rx,
            binary_rx,
            plugins: self.plugins,
            session: ChatSession::default(),
        })
    }
}

pub struct ClientRuntime {
    connection: ClientConnection,
    identity: ClientIdentity,
    inbound_rx: mpsc::UnboundedReceiver<IncomingServerPacket>,
    binary_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    plugins: PluginRegistry,
    session: ChatSession,
}

pub enum IncomingRuntimeMessage {
    Packet(IncomingServerPacket),
    Binary(Vec<u8>),
}

impl ClientRuntime {
    pub fn identity(&self) -> &ClientIdentity {
        &self.identity
    }

    pub fn connection(&self) -> &ClientConnection {
        &self.connection
    }

    pub async fn recv_next_packet(&mut self) -> Option<IncomingServerPacket> {
        self.inbound_rx.recv().await
    }

    pub async fn recv_binary_frame(&mut self) -> Option<Vec<u8>> {
        self.binary_rx.recv().await
    }

    pub async fn recv_next_message(&mut self) -> Option<IncomingRuntimeMessage> {
        tokio::select! {
            maybe_packet = self.inbound_rx.recv() => {
                maybe_packet.map(IncomingRuntimeMessage::Packet)
            }
            maybe_binary = self.binary_rx.recv() => {
                maybe_binary.map(IncomingRuntimeMessage::Binary)
            }
        }
    }

    pub fn session(&self) -> &ChatSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut ChatSession {
        &mut self.session
    }

    pub fn spawn_heartbeat_loop(&self) -> JoinHandle<()> {
        let connection = self.connection.clone();
        let identity = self.identity.clone();
        let interval_sec = self.identity.heartbeat_interval_sec.max(1);

        tokio::spawn(async move {
            let mut ticker = time::interval(Duration::from_secs(interval_sec));
            loop {
                ticker.tick().await;
                let packet = build_heartbeat_packet(&identity);
                if connection.send_json_packet(&packet).is_err() {
                    break;
                }
            }
        })
    }

    pub fn send_heartbeat(&self) -> anyhow::Result<()> {
        let packet = build_heartbeat_packet(&self.identity);
        self.connection.send_json_packet(&packet)
    }

    pub fn send_text(
        &self,
        target_group_name: Option<String>,
        target_client_name: Option<String>,
        content: impl Into<String>,
    ) -> anyhow::Result<()> {
        let packet = build_text_event_packet(
            &self.identity,
            target_group_name.or_else(|| Some(self.session.resolved_group_name(&self.identity))),
            target_client_name.or_else(|| self.session.client_name.clone()),
            content,
        );
        self.connection.send_json_packet(&packet)
    }

    pub fn send_json(
        &self,
        target_group_name: Option<String>,
        target_client_name: Option<String>,
        content: Value,
    ) -> anyhow::Result<()> {
        let packet = build_json_event_packet(
            &self.identity,
            target_group_name.or_else(|| Some(self.session.resolved_group_name(&self.identity))),
            target_client_name.or_else(|| self.session.client_name.clone()),
            content,
        );
        self.connection.send_json_packet(&packet)
    }

    pub fn send_command(
        &self,
        target_group_name: Option<String>,
        target_client_name: Option<String>,
        command: impl Into<String>,
        timeout_sec: Option<u64>,
    ) -> anyhow::Result<()> {
        let packet = build_command_event_packet(
            &self.identity,
            target_group_name.or_else(|| Some(self.session.resolved_group_name(&self.identity))),
            target_client_name.or_else(|| self.session.client_name.clone()),
            command,
            timeout_sec,
        );
        self.connection.send_json_packet(&packet)
    }

    pub async fn dispatch_next(&mut self) -> anyhow::Result<bool> {
        let Some(packet) = self.recv_next_packet().await else {
            return Ok(false);
        };

        self.dispatch_packet(packet).await?;
        Ok(true)
    }

    pub async fn dispatch_packet(&mut self, packet: IncomingServerPacket) -> anyhow::Result<()> {

        match packet {
            IncomingServerPacket::Event(event) => {
                let ctx = PluginContext {
                    identity: self.identity.clone(),
                    connection: self.connection.clone(),
                };
                self.plugins.dispatch(&ctx, &event).await?;
            }
            IncomingServerPacket::Error(error) => {
                log_server_error(&error);
            }
            IncomingServerPacket::InitAck(_)
            | IncomingServerPacket::HeartbeatAck(_)
            | IncomingServerPacket::Ack(_) => {}
        }

        Ok(())
    }

    pub async fn run_forever(&mut self) -> anyhow::Result<()> {
        while self.dispatch_next().await? {}
        Ok(())
    }
}

async fn wait_for_init_ack(
    inbound_rx: &mut mpsc::UnboundedReceiver<IncomingServerPacket>,
) -> anyhow::Result<y2m_common::InitAckPacket> {
    loop {
        match inbound_rx.recv().await {
            Some(IncomingServerPacket::InitAck(packet)) => return Ok(packet),
            Some(IncomingServerPacket::Error(packet)) => {
                return Err(ClientCoreError::InitRejected(packet.payload.message).into())
            }
            Some(_) => continue,
            None => return Err(ClientCoreError::InitChannelClosed.into()),
        }
    }
}

fn log_server_error(packet: &ErrorPacket) {
    warn!(
        request_id = %packet.request_id,
        code = ?packet.payload.code,
        message = %packet.payload.message,
        "server returned error"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_send_defaults_to_identity_group() {
        let identity = ClientIdentity {
            connection_id: uuid::Uuid::nil(),
            group_name: "group-a".to_string(),
            client_name: "alice".to_string(),
            heartbeat_interval_sec: 60,
            heartbeat_timeout_sec: 150,
        };

        let packet = build_text_event_packet(&identity, Some("group-a".to_string()), None, "hello");
        assert_eq!(
            packet.target.and_then(|target| target.group_name),
            Some("group-a".to_string())
        );
    }
}
