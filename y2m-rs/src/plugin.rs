use std::sync::Arc;

use async_trait::async_trait;
use y2m_client_core::{Plugin, PluginContext};
use y2m_common::{EventPacket, EventType};

use crate::{
    printer::cprintln,
    state::{command_results::schedule_command_result_flush, ConsoleState},
    util::{format_sender_context_line, sanitize_terminal_controls},
};

#[derive(Clone)]
pub(crate) struct ConsolePlugin {
    pub(crate) state: Arc<ConsoleState>,
}

const CONSOLE_EVENTS: &[EventType] = &[
    EventType::Text, EventType::Command, EventType::Json, EventType::CommandResult,
    EventType::FileOffer, EventType::FileAccept, EventType::FileReject,
    EventType::FileComplete, EventType::FileAbort,
];

#[async_trait]
impl Plugin for ConsolePlugin {
    fn name(&self) -> &'static str { "console" }

    fn supports(&self) -> &'static [EventType] { CONSOLE_EVENTS }

    async fn on_event(&self, ctx: &PluginContext, packet: &EventPacket) -> anyhow::Result<()> {
        let from = packet.source.as_ref().and_then(|e| e.client_name.as_deref()).unwrap_or("unknown");
        let group = packet.source.as_ref().and_then(|e| e.group_name.as_deref()).unwrap_or("unknown");
        dispatch_event(&self.state, ctx, packet, from, group).await
    }
}

async fn dispatch_event(
    state: &Arc<ConsoleState>,
    ctx: &PluginContext,
    packet: &EventPacket,
    from: &str,
    group: &str,
) -> anyhow::Result<()> {
    match packet.payload.event_type {
        EventType::Text => {
            let content = packet.payload.content.as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| packet.payload.content.to_string());
            if let Some(ctx) = format_sender_context_line(&packet.payload.metadata) {
                cprintln!("[{group}][{from} | {ctx}] {content}");
            } else {
                cprintln!("[{group}][{from}] {content}");
            }
        }
        EventType::Json => {
            let line = packet.payload.content.to_string();
            if let Some(ctx) = format_sender_context_line(&packet.payload.metadata) {
                cprintln!("[{group}][{from} | {ctx}] {line}");
            } else {
                cprintln!("[{group}][{from}] {line}");
            }
        }
        EventType::CommandResult => {
            let meta = &packet.payload.metadata;
            let exit_code = meta.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(0);
            let stdout = sanitize_terminal_controls(
                meta.get("stdout").and_then(|v| v.as_str()).unwrap_or(""),
            );
            let stderr = sanitize_terminal_controls(
                meta.get("stderr").and_then(|v| v.as_str()).unwrap_or(""),
            );
            let stdout = stdout.trim_end_matches('\n').to_string();
            let stderr = stderr.trim_end_matches('\n').to_string();
            let duration_ms = meta.get("durationMs").and_then(|v| v.as_u64()).unwrap_or(0);
            let rid = packet.request_id.clone();
            let sender_ctx = format_sender_context_line(meta);
            let gen = state.merge_command_result_snapshot(
                &rid, group, from, exit_code, stdout, stderr, duration_ms, sender_ctx,
            );
            schedule_command_result_flush(Arc::clone(state), rid, gen);
        }
        EventType::FileOffer => {
            cprintln!("[{group}][{from}] file_offer {}", packet.payload.metadata);
            state.handle_file_offer(ctx, packet)?;
        }
        EventType::FileAccept => {
            cprintln!("[{group}][{from}] file_accept {}", packet.payload.metadata);
            state.handle_outgoing_file_accept(ctx, packet).await?;
        }
        EventType::FileReject => {
            cprintln!("[{group}][{from}] file_reject {}", packet.payload.metadata);
            state.handle_file_reject(packet)?;
        }
        EventType::FileComplete => {
            cprintln!("[{group}][{from}] file_complete {}", packet.payload.metadata);
            state.handle_file_complete(ctx, packet)?;
        }
        EventType::FileAbort => {
            cprintln!("[{group}][{from}] file_abort {}", packet.payload.metadata);
            state.handle_file_abort(packet)?;
        }
        EventType::Command => state.handle_command_event(ctx, packet).await?,
    }
    Ok(())
}
