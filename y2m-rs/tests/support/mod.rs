#![allow(dead_code)]

pub mod cli;

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::{net::TcpListener, sync::mpsc, time::timeout};
use y2m_client_core::{
    build_command_result_packet, ClientConfig, ClientCore, ClientRuntime, Plugin, PluginContext,
};
use y2m_common::{EventPacket, EventType};
use y2m_server::ServerConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct ReceivedEvent {
    pub group: String,
    pub from: String,
    pub event_type: EventType,
    pub content: Value,
    pub metadata: Value,
}

/// Assert every key in `required` matches `meta`; extra keys (e.g. sender envelope) are allowed.
pub fn assert_metadata_superset(meta: &Value, required: Value) {
    let m = meta.as_object().expect("metadata must be object");
    let req = required.as_object().expect("required must be object");
    for (k, ev) in req {
        let av = m.get(k).unwrap_or_else(|| panic!("missing metadata key {k}"));
        assert_eq!(av, ev, "metadata[{k}]");
    }
}

pub fn assert_sender_envelope_keys(meta: &Value) {
    let m = meta.as_object().expect("metadata object");
    for k in ["senderIp", "senderMac", "senderUser", "senderOs"] {
        assert!(m.contains_key(k), "expected sender envelope key {k}");
    }
}

pub struct CaptureEventPlugin {
    pub tx: mpsc::UnboundedSender<ReceivedEvent>,
    pub supported: &'static [EventType],
}

#[async_trait]
impl Plugin for CaptureEventPlugin {
    fn name(&self) -> &'static str {
        "capture-event"
    }

    fn supports(&self) -> &'static [EventType] {
        self.supported
    }

    async fn on_event(
        &self,
        _ctx: &PluginContext,
        packet: &EventPacket,
    ) -> anyhow::Result<()> {
        let from = packet
            .source
            .as_ref()
            .and_then(|endpoint| endpoint.client_name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let group = packet
            .target
            .as_ref()
            .and_then(|endpoint| endpoint.group_name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let _ = self.tx.send(ReceivedEvent {
            group,
            from,
            event_type: packet.payload.event_type,
            content: packet.payload.content.clone(),
            metadata: packet.payload.metadata.clone(),
        });

        Ok(())
    }
}

pub struct CommandResponderPlugin;

#[async_trait]
impl Plugin for CommandResponderPlugin {
    fn name(&self) -> &'static str {
        "command-responder"
    }

    fn supports(&self) -> &'static [EventType] {
        &[EventType::Command]
    }

    async fn on_event(
        &self,
        ctx: &PluginContext,
        packet: &EventPacket,
    ) -> anyhow::Result<()> {
        let target_group = packet
            .source
            .as_ref()
            .and_then(|endpoint| endpoint.group_name.clone());
        let target_client = packet
            .source
            .as_ref()
            .and_then(|endpoint| endpoint.client_name.clone());
        let command = packet
            .payload
            .content
            .as_str()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| packet.payload.content.to_string());

        let response = build_command_result_packet(
            &ctx.identity,
            target_group,
            target_client,
            packet.request_id.clone(),
            0,
            format!("echo: {command}"),
            "",
            1,
        );
        ctx.connection.send_json_packet(&response)?;
        Ok(())
    }
}

pub async fn spawn_server() -> anyhow::Result<(
    tokio::task::JoinHandle<anyhow::Result<()>>,
    String,
)> {
    spawn_server_with_config(ServerConfig::default()).await
}

pub async fn spawn_server_with_config(
    config: ServerConfig,
) -> anyhow::Result<(tokio::task::JoinHandle<anyhow::Result<()>>, String)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server_task =
        tokio::spawn(async move { y2m_server::serve_with_listener_and_config(listener, config).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    Ok((server_task, format!("ws://{addr}/ws")))
}

pub async fn connect_runtime(
    server_url: String,
    group_name: &str,
    client_name: &str,
    plugins: Vec<Arc<dyn Plugin>>,
) -> anyhow::Result<ClientRuntime> {
    let mut core = ClientCore::new(ClientConfig {
        server_url,
        group_name: Some(group_name.to_string()),
        client_name: Some(client_name.to_string()),
        ..Default::default()
    });

    for plugin in plugins {
        core.plugin_registry_mut().register(plugin);
    }

    core.connect().await
}

pub fn spawn_dispatch_loop(
    mut runtime: ClientRuntime,
) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        loop {
            if !runtime.dispatch_next().await? {
                break;
            }
        }
        Ok(())
    })
}

pub async fn recv_event(
    rx: &mut mpsc::UnboundedReceiver<ReceivedEvent>,
) -> anyhow::Result<ReceivedEvent> {
    Ok(timeout(Duration::from_secs(5), rx.recv())
        .await?
        .expect("expected captured event"))
}

#[allow(dead_code)]
pub fn json_message(value: &str) -> Value {
    json!({ "message": value })
}
