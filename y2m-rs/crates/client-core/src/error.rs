use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientCoreError {
    #[error("unexpected server packet kind")]
    UnexpectedPacketKind,
    #[error("connection closed before init completed")]
    InitChannelClosed,
    #[error("server init failed: {0}")]
    InitRejected(String),
    #[error("websocket writer channel is closed")]
    OutboundChannelClosed,
}
