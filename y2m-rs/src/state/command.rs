use std::time::{Duration, Instant};

use tokio::process::Command as TokioCommand;
use y2m_client_core::{build_command_result_packet, PluginContext};
use y2m_common::EventPacket;
#[cfg(not(windows))]
use y2m_common::{default_shell_arg, default_shell_program};

use crate::{printer::cprintln, util::format_sender_context_line};

use super::ConsoleState;

impl ConsoleState {
    pub(crate) async fn handle_command_event(&self, ctx: &PluginContext, packet: &EventPacket) -> anyhow::Result<()> {
        let (command, timeout_sec, target_group, target_client) = extract_command_params(packet);
        cprintln!(
            "收到命令请求: from=[{}][{}], timeout={}s, command={}",
            target_group.as_deref().unwrap_or("unknown"),
            target_client.as_deref().unwrap_or("unknown"),
            timeout_sec, command
        );
        if let Some(ctx) = format_sender_context_line(&packet.payload.metadata) {
            cprintln!("  发送方终端: {ctx}");
        }
        let (exit_code, stdout, stderr, duration_ms) =
            execute_command(&command, timeout_sec).await;
        let result = build_command_result_packet(
            &ctx.identity, target_group, target_client,
            packet.request_id.clone(), exit_code, stdout, stderr, duration_ms,
        );
        ctx.connection.send_json_packet(&result)?;
        Ok(())
    }
}

fn extract_command_params(packet: &EventPacket) -> (String, u64, Option<String>, Option<String>) {
    let command = packet.payload.content.as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| packet.payload.content.to_string());
    let timeout_sec = packet.payload.metadata.get("timeoutSec")
        .and_then(|v| v.as_u64()).unwrap_or(30).max(1);
    let target_group = packet.source.as_ref().and_then(|e| e.group_name.clone());
    let target_client = packet.source.as_ref().and_then(|e| e.client_name.clone());
    (command, timeout_sec, target_group, target_client)
}

async fn execute_command(command: &str, timeout_sec: u64) -> (i32, String, String, u64) {
    use std::process::Stdio;
    let started_at = Instant::now();
    let (program, args) = shell_args(command);
    let mut process = TokioCommand::new(&program);
    process.args(&args)
        .stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let result = tokio::time::timeout(Duration::from_secs(timeout_sec), process.output()).await;
    let duration_ms = started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
    match result {
        Ok(Ok(output)) => (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms,
        ),
        Ok(Err(e)) => (-1, String::new(), format!("failed to execute command: {e}"), duration_ms),
        Err(_) => (-1, String::new(), format!("command timed out after {timeout_sec}s"), duration_ms),
    }
}

fn shell_args(command: &str) -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        // Prepend encoding fix so Chinese output doesn't garble.
        // pwsh has no -OutputEncoding CLI flag; we set it inside the session instead.
        let wrapped = format!(
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; {}",
            command
        );
        (
            "pwsh".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                wrapped,
            ],
        )
    }
    #[cfg(not(windows))]
    {
        (default_shell_program().to_string(), vec![default_shell_arg().to_string(), command.to_string()])
    }
}
