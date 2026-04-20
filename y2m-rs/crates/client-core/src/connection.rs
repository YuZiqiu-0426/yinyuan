use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    error::ClientCoreError,
    protocol::{decode_server_packet, IncomingServerPacket},
};

#[derive(Clone)]
pub struct ClientConnection {
    outbound_tx: mpsc::UnboundedSender<Message>,
}

impl ClientConnection {
    pub async fn connect(
        server_url: &str,
    ) -> anyhow::Result<(
        Self,
        mpsc::UnboundedReceiver<IncomingServerPacket>,
        mpsc::UnboundedReceiver<Vec<u8>>,
    )> {
        let (stream, _) = connect_async(server_url).await?;
        let (mut writer, mut reader) = stream.split();

        let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<Message>();
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel::<IncomingServerPacket>();
        let (binary_tx, binary_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        tokio::spawn(async move {
            while let Some(message) = outbound_rx.recv().await {
                if writer.send(message).await.is_err() {
                    break;
                }
            }
        });

        tokio::spawn(async move {
            while let Some(result) = reader.next().await {
                let message = match result {
                    Ok(message) => message,
                    Err(_) => break,
                };

                match message {
                    Message::Text(text) => {
                        if let Ok(packet) = decode_server_packet(text.as_str()) {
                            let _ = inbound_tx.send(packet);
                        }
                    }
                    Message::Binary(bytes) => {
                        let _ = binary_tx.send(bytes.to_vec());
                    }
                    Message::Close(_) => break,
                    Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
                }
            }
        });

        Ok((Self { outbound_tx }, inbound_rx, binary_rx))
    }

    pub fn send_json_packet<T: Serialize>(&self, packet: &T) -> anyhow::Result<()> {
        let text = serde_json::to_string(packet)?;
        self.outbound_tx
            .send(Message::Text(text.into()))
            .map_err(|_| ClientCoreError::OutboundChannelClosed.into())
    }

    pub fn send_binary(&self, bytes: Vec<u8>) -> anyhow::Result<()> {
        self.outbound_tx
            .send(Message::Binary(bytes.into()))
            .map_err(|_| ClientCoreError::OutboundChannelClosed.into())
    }
}
