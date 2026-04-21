use y2m_common::{AckPacket, Endpoint, EventPacket, PacketKind, PROTOCOL_VERSION};

use crate::{
    error::ServerError,
    session::{SessionRecord, SessionStore},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteMode {
    Unicast,
    Broadcast,
}

#[derive(Debug, Clone)]
pub struct RouteResult {
    pub mode: RouteMode,
    pub packet: EventPacket,
    pub recipients: Vec<SessionRecord>,
}

#[derive(Debug, Clone)]
pub struct AckRouteResult {
    pub packet: AckPacket,
    pub recipient: SessionRecord,
}

pub async fn route_event(
    store: &SessionStore,
    sender: &SessionRecord,
    mut packet: EventPacket,
) -> Result<RouteResult, ServerError> {
    if packet.version != PROTOCOL_VERSION {
        return Err(ServerError::UnsupportedVersion(packet.version));
    }

    if packet.kind != PacketKind::Event {
        return Err(ServerError::InvalidPacketKind);
    }

    packet.source = Some(sender.endpoint());

    let target_group = packet
        .target
        .as_ref()
        .and_then(|target| target.group_name.clone())
        .unwrap_or_else(|| sender.group_name.clone());
    let target_client = packet
        .target
        .as_ref()
        .and_then(|target| target.client_name.clone());

    packet.target = Some(Endpoint {
        group_name: Some(target_group.clone()),
        client_name: target_client.clone(),
    });

    match target_client {
        Some(client_name) => {
            let recipient = store.resolve_unicast(&target_group, &client_name).await?;
            Ok(RouteResult {
                mode: RouteMode::Unicast,
                packet,
                recipients: vec![recipient],
            })
        }
        None => {
            let recipients = store
                .resolve_broadcast(&target_group, sender.connection_id)
                .await?;
            Ok(RouteResult {
                mode: RouteMode::Broadcast,
                packet,
                recipients,
            })
        }
    }
}

pub async fn route_ack(
    store: &SessionStore,
    sender: &SessionRecord,
    mut packet: AckPacket,
) -> Result<AckRouteResult, ServerError> {
    if packet.version != PROTOCOL_VERSION {
        return Err(ServerError::UnsupportedVersion(packet.version));
    }

    if packet.kind != PacketKind::Ack {
        return Err(ServerError::InvalidPacketKind);
    }

    packet.source = Some(sender.endpoint());

    let target_group = packet
        .target
        .as_ref()
        .and_then(|target| target.group_name.clone())
        .unwrap_or_else(|| sender.group_name.clone());
    let target_client = packet
        .target
        .as_ref()
        .and_then(|target| target.client_name.clone())
        .ok_or_else(|| ServerError::ClientNotFound {
            group_name: target_group.clone(),
            client_name: String::new(),
        })?;

    packet.target = Some(Endpoint {
        group_name: Some(target_group.clone()),
        client_name: Some(target_client.clone()),
    });

    let recipient = store.resolve_unicast(&target_group, &target_client).await?;
    Ok(AckRouteResult { packet, recipient })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::sync::mpsc;
    use y2m_common::{CapabilitySet, Endpoint, EventPayload, EventType, Packet};

    use super::*;

    #[tokio::test]
    async fn text_broadcast_defaults_to_sender_group() {
        let store = SessionStore::new();
        let (sender_tx, _sender_rx) = mpsc::unbounded_channel();
        let sender = store
            .register(
                Some("group-a"),
                Some("alice"),
                None,
                CapabilitySet::default(),
                sender_tx,
            )
            .await
            .unwrap();
        let (recipient_tx, _recipient_rx) = mpsc::unbounded_channel();
        let recipient = store
            .register(
                Some("group-a"),
                Some("bob"),
                None,
                CapabilitySet::default(),
                recipient_tx,
            )
            .await
            .unwrap();

        let packet = Packet::new(
            PacketKind::Event,
            "req-1",
            1,
            None,
            None,
            EventPayload {
                event_type: EventType::Text,
                content: json!("hello"),
                metadata: json!({}),
            },
        );

        let result = route_event(&store, &sender, packet).await.unwrap();
        assert_eq!(result.mode, RouteMode::Broadcast);
        assert_eq!(result.recipients.len(), 1);
        assert_eq!(result.recipients[0].connection_id, recipient.connection_id);
        assert_eq!(
            result.packet.target.unwrap().group_name.as_deref(),
            Some("group-a")
        );
    }

    #[tokio::test]
    async fn command_broadcast_targets_group_except_sender() {
        let store = SessionStore::new();
        let (sender_tx, _sender_rx) = mpsc::unbounded_channel();
        let sender = store
            .register(
                Some("group-a"),
                Some("alice"),
                None,
                CapabilitySet::default(),
                sender_tx,
            )
            .await
            .unwrap();
        let (recipient_tx, _recipient_rx) = mpsc::unbounded_channel();
        let recipient = store
            .register(
                Some("group-a"),
                Some("bob"),
                None,
                CapabilitySet::default(),
                recipient_tx,
            )
            .await
            .unwrap();

        let packet = Packet::new(
            PacketKind::Event,
            "req-2",
            1,
            None,
            Some(Endpoint {
                group_name: Some("group-a".to_string()),
                client_name: None,
            }),
            EventPayload {
                event_type: EventType::Command,
                content: json!("whoami"),
                metadata: json!({}),
            },
        );

        let result = route_event(&store, &sender, packet).await.unwrap();
        assert_eq!(result.mode, RouteMode::Broadcast);
        assert_eq!(result.recipients.len(), 1);
        assert_eq!(result.recipients[0].connection_id, recipient.connection_id);
    }

    #[tokio::test]
    async fn file_offer_broadcast_targets_group_except_sender() {
        let store = SessionStore::new();
        let (sender_tx, _sender_rx) = mpsc::unbounded_channel();
        let sender = store
            .register(
                Some("group-a"),
                Some("alice"),
                None,
                CapabilitySet::default(),
                sender_tx,
            )
            .await
            .unwrap();
        let (recipient_tx, _recipient_rx) = mpsc::unbounded_channel();
        let recipient = store
            .register(
                Some("group-a"),
                Some("bob"),
                None,
                CapabilitySet::default(),
                recipient_tx,
            )
            .await
            .unwrap();

        let packet = Packet::new(
            PacketKind::Event,
            "req-3",
            1,
            None,
            Some(Endpoint {
                group_name: Some("group-a".to_string()),
                client_name: None,
            }),
            EventPayload {
                event_type: EventType::FileOffer,
                content: serde_json::Value::Null,
                metadata: json!({
                    "fileId": "file-1",
                    "fileName": "a.txt",
                    "fileSize": 1
                }),
            },
        );

        let result = route_event(&store, &sender, packet).await.unwrap();
        assert_eq!(result.mode, RouteMode::Broadcast);
        assert_eq!(result.recipients.len(), 1);
        assert_eq!(result.recipients[0].connection_id, recipient.connection_id);
    }
}
