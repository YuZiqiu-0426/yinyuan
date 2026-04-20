use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    extract::{
        connect_info::ConnectInfo,
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::TcpListener,
    task::JoinHandle,
    sync::mpsc::{self, UnboundedSender},
};
use tracing::{info, warn};
use uuid::Uuid;
use y2m_common::{BinaryChunkHeader, EventPayload, EventType, HeartbeatPayload, Packet, PacketKind};
use serde_json::{json, Map, Value};

use crate::{
    decode_text_packet, encode_packet, handle_init, route_ack, route_event, ConnectionMessage,
    IncomingTextPacket, ServerConfig, ServerError, SessionRecord, SessionStore, TransferRegistry,
};

#[derive(Clone)]
struct AppState {
    store: Arc<SessionStore>,
    transfers: Arc<TransferRegistry>,
    config: ServerConfig,
}

pub async fn serve(addr: SocketAddr) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    serve_with_listener(listener).await
}

pub async fn serve_with_listener(listener: TcpListener) -> anyhow::Result<()> {
    serve_with_listener_and_config(listener, ServerConfig::default()).await
}

pub async fn serve_with_listener_and_config(
    listener: TcpListener,
    config: ServerConfig,
) -> anyhow::Result<()> {
    let addr = listener.local_addr()?;
    let state = Arc::new(AppState {
        store: Arc::new(SessionStore::new()),
        transfers: Arc::new(TransferRegistry::new()),
        config,
    });
    let sweeper = spawn_heartbeat_timeout_task(state.clone());

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    info!(%addr, "y2m server listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    sweeper.abort();

    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, addr))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>, remote_addr: SocketAddr) {
    let (mut socket_sender, mut socket_receiver) = socket.split();
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<ConnectionMessage>();

    let write_task = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            let should_close = matches!(message, ConnectionMessage::Close);
            let result = match message {
                ConnectionMessage::Text(text) => socket_sender.send(Message::Text(text.into())).await,
                ConnectionMessage::Binary(bytes) => {
                    socket_sender.send(Message::Binary(bytes.into())).await
                }
                ConnectionMessage::Close => socket_sender.send(Message::Close(None)).await,
            };

            if result.is_err() {
                break;
            }
            if should_close {
                break;
            }
        }
    });

    let mut session: Option<SessionRecord> = None;

    while let Some(result) = socket_receiver.next().await {
        let message = match result {
            Ok(message) => message,
            Err(error) => {
                warn!(%remote_addr, %error, "websocket receive error");
                break;
            }
        };

        match message {
            Message::Text(text) => {
                if handle_text_message(
                    &state,
                    &mut session,
                    &outbound_tx,
                    remote_addr,
                    text.as_str(),
                )
                .await
                .is_err()
                {
                    break;
                }
            }
            Message::Binary(bytes) => {
                if let Err(error) = handle_binary_message(
                    &state,
                    session.as_ref(),
                    bytes.to_vec(),
                )
                .await
                {
                    let packet = error.to_packet(
                        Uuid::new_v4().to_string(),
                        session.as_ref().map(|s| s.endpoint()),
                    );
                    let _ = send_json_packet(&outbound_tx, &packet);
                    warn!(%remote_addr, %error, "failed to handle binary frame");
                }
            }
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) => {}
        }
    }

    if let Some(active_session) = session {
        cleanup_disconnected_session(&state, active_session, "peer disconnected").await;
    }

    drop(outbound_tx);
    let _ = write_task.await;
}

fn spawn_heartbeat_timeout_task(state: Arc<AppState>) -> JoinHandle<()> {
    let interval = Duration::from_secs(state.config.heartbeat_timeout_sec.clamp(1, 30));
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            let expired = state
                .store
                .expired_connection_ids(state.config.heartbeat_timeout_sec)
                .await;
            for connection_id in expired {
                let Some(session) = state.store.remove_connection(connection_id).await else {
                    continue;
                };
                let packet = ServerError::HeartbeatTimeout
                    .to_packet(Uuid::new_v4().to_string(), Some(session.endpoint()));
                let _ = send_json_packet(&session.outbound_tx, &packet);
                let _ = session.outbound_tx.send(ConnectionMessage::Close);
                cleanup_related_transfers(&state, &session, "heartbeat timeout").await;
            }
        }
    })
}

async fn cleanup_disconnected_session(
    state: &Arc<AppState>,
    session: SessionRecord,
    reason: &str,
) {
    let _ = state.store.remove_connection(session.connection_id).await;
    cleanup_related_transfers(state, &session, reason).await;
}

async fn cleanup_related_transfers(
    state: &Arc<AppState>,
    session: &SessionRecord,
    reason: &str,
) {
    let removed = state
        .transfers
        .remove_by_connection(session.connection_id)
        .await;
    for transfer in removed {
        let peer_connection_id = if transfer.sender_connection_id == session.connection_id {
            transfer.receiver_connection_id
        } else {
            transfer.sender_connection_id
        };
        let Some(peer) = state.store.resolve_connection(peer_connection_id).await else {
            continue;
        };
        let abort_packet = Packet::new(
            PacketKind::Event,
            Uuid::new_v4().to_string(),
            crate::now_timestamp(),
            Some(session.endpoint()),
            Some(peer.endpoint()),
            EventPayload {
                event_type: EventType::FileAbort,
                content: Value::Null,
                metadata: json!({
                    "fileId": transfer.file_id,
                    "reason": reason,
                }),
            },
        );
        let _ = send_json_packet(&peer.outbound_tx, &abort_packet);
    }
}

async fn handle_text_message(
    state: &Arc<AppState>,
    session: &mut Option<SessionRecord>,
    outbound_tx: &UnboundedSender<ConnectionMessage>,
    remote_addr: SocketAddr,
    text: &str,
) -> Result<(), ()> {
    let incoming = match decode_text_packet(text) {
        Ok(packet) => packet,
        Err(error) => {
            let packet = error.to_packet(Uuid::new_v4().to_string(), session.as_ref().map(|s| s.endpoint()));
            let _ = send_json_packet(outbound_tx, &packet);
            return Ok(());
        }
    };

    match incoming {
        IncomingTextPacket::Init(packet) => {
            if session.is_some() {
                let error = ServerError::InvalidPacketKind;
                let packet = error.to_packet(packet.request_id, session.as_ref().map(|s| s.endpoint()));
                let _ = send_json_packet(outbound_tx, &packet);
                return Ok(());
            }

            match handle_init(
                &state.store,
                &state.config,
                packet,
                Some(remote_addr.to_string()),
                outbound_tx.clone(),
            )
            .await
            {
                Ok((registered, ack)) => {
                    *session = Some(registered);
                    let _ = send_json_packet(outbound_tx, &ack);
                }
                Err(error) => {
                    let packet = error.to_packet(
                        Uuid::new_v4().to_string(),
                        session.as_ref().map(|s| s.endpoint()),
                    );
                    let _ = send_json_packet(outbound_tx, &packet);
                }
            }
        }
        IncomingTextPacket::Heartbeat(packet) => {
            let Some(active_session) = session.as_ref() else {
                let error = ServerError::InvalidPacketKind;
                let packet = error.to_packet(packet.request_id, None);
                let _ = send_json_packet(outbound_tx, &packet);
                return Ok(());
            };

            state
                .store
                .touch_heartbeat(active_session.connection_id)
                .await;

            let ack = Packet::new(
                PacketKind::HeartbeatAck,
                packet.request_id,
                crate::now_timestamp(),
                Some(y2m_common::Endpoint::server()),
                Some(active_session.endpoint()),
                HeartbeatPayload::default(),
            );
            let _ = send_json_packet(outbound_tx, &ack);
        }
        IncomingTextPacket::Event(packet) => {
            let Some(active_session) = session.as_ref() else {
                let error = ServerError::InvalidPacketKind;
                let packet = error.to_packet(packet.request_id, None);
                let _ = send_json_packet(outbound_tx, &packet);
                return Ok(());
            };

            match route_event(&state.store, active_session, packet).await {
                Ok(routed) => {
                    if let Err(error) =
                        apply_transfer_event_side_effects(state, active_session, &routed).await
                    {
                        let packet =
                            error.to_packet(Uuid::new_v4().to_string(), Some(active_session.endpoint()));
                        let _ = send_json_packet(outbound_tx, &packet);
                        return Ok(());
                    }

                    for recipient in routed.recipients {
                        let _ = send_json_packet(&recipient.outbound_tx, &routed.packet);
                    }
                }
                Err(error) => {
                    let packet =
                        error.to_packet(Uuid::new_v4().to_string(), Some(active_session.endpoint()));
                    let _ = send_json_packet(outbound_tx, &packet);
                }
            }
        }
        IncomingTextPacket::Ack(packet) => {
            let Some(active_session) = session.as_ref() else {
                let error = ServerError::InvalidPacketKind;
                let packet = error.to_packet(packet.request_id, None);
                let _ = send_json_packet(outbound_tx, &packet);
                return Ok(());
            };

            match route_ack(&state.store, active_session, packet).await {
                Ok(routed) => {
                    if matches!(
                        routed.packet.payload.ref_type,
                        Some(EventType::FileComplete)
                    ) {
                        state
                            .transfers
                            .remove_by_request_id(&routed.packet.request_id)
                            .await;
                    }

                    let _ = send_json_packet(&routed.recipient.outbound_tx, &routed.packet);
                }
                Err(error) => {
                    let packet =
                        error.to_packet(Uuid::new_v4().to_string(), Some(active_session.endpoint()));
                    let _ = send_json_packet(outbound_tx, &packet);
                }
            }
        }
        IncomingTextPacket::Error(_) => {}
    }

    Ok(())
}

async fn apply_transfer_event_side_effects(
    state: &Arc<AppState>,
    active_session: &SessionRecord,
    routed: &crate::RouteResult,
) -> Result<(), ServerError> {
    let Some(metadata) = routed.packet.payload.metadata.as_object() else {
        return Ok(());
    };

    let Some(file_id) = metadata
        .get("fileId")
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
    else {
        return Ok(());
    };

    match routed.packet.payload.event_type {
        EventType::FileOffer => {
            validate_file_offer_limits(&state.config, metadata, file_id)?;
            if let Some(recipient) = routed.recipients.first() {
                state
                    .transfers
                    .create_offer(
                        file_id,
                        routed.packet.request_id.clone(),
                        active_session.connection_id,
                        recipient.connection_id,
                    )
                    .await;
            }
        }
        EventType::FileAccept => {
            state
                .transfers
                .mark_accepted(file_id, active_session.connection_id)
                .await?;
        }
        EventType::FileReject | EventType::FileAbort => {
            state.transfers.remove_by_file_id(file_id).await;
        }
        EventType::FileComplete => {
            state
                .transfers
                .update_request_id(file_id, routed.packet.request_id.clone())
                .await?;
        }
        EventType::Text
        | EventType::Command
        | EventType::CommandResult
        | EventType::Json => {}
    }

    Ok(())
}

fn validate_file_offer_limits(
    config: &ServerConfig,
    metadata: &Map<String, Value>,
    file_id: Uuid,
) -> Result<(), ServerError> {
    let file_name = metadata
        .get("fileName")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    let file_id = Some(file_id.to_string());

    if let Some(file_size) = metadata.get("fileSize").and_then(|value| value.as_u64()) {
        if file_size > config.max_file_size {
            return Err(ServerError::FileTooLarge {
                file_id: file_id.clone(),
                file_name: file_name.clone(),
                field_name: "fileSize".to_string(),
                actual: file_size,
                max: config.max_file_size,
            });
        }
    }

    if let Some(chunk_size) = metadata.get("chunkSize").and_then(|value| value.as_u64()) {
        if chunk_size > config.max_chunk_size as u64 {
            return Err(ServerError::FileTooLarge {
                file_id,
                file_name,
                field_name: "chunkSize".to_string(),
                actual: chunk_size,
                max: config.max_chunk_size as u64,
            });
        }
    }

    Ok(())
}

async fn handle_binary_message(
    state: &Arc<AppState>,
    session: Option<&SessionRecord>,
    bytes: Vec<u8>,
) -> Result<(), ServerError> {
    let Some(active_session) = session else {
        return Err(ServerError::InvalidPacketKind);
    };

    let (header, _) = BinaryChunkHeader::decode(&bytes).ok_or(ServerError::InvalidBinaryChunk)?;
    let transfer = state
        .transfers
        .get_by_file_id(header.file_id)
        .await
        .ok_or(ServerError::FileTransferNotAccepted)?;

    if !transfer.accepted || transfer.sender_connection_id != active_session.connection_id {
        return Err(ServerError::FileTransferNotAccepted);
    }

    let Some(receiver) = state
        .store
        .resolve_connection(transfer.receiver_connection_id)
        .await
    else {
        return Err(ServerError::ClientNotFound {
            group_name: String::new(),
            client_name: String::new(),
        });
    };

    receiver
        .outbound_tx
        .send(ConnectionMessage::Binary(bytes))
        .map_err(|_| ServerError::FileTransferNotAccepted)?;

    Ok(())
}

fn send_json_packet<T: serde::Serialize>(
    outbound_tx: &UnboundedSender<ConnectionMessage>,
    packet: &T,
) -> anyhow::Result<()> {
    let text = encode_packet(packet)?;
    outbound_tx
        .send(ConnectionMessage::Text(text))
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}
