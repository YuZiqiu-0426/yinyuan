use std::{sync::Arc, time::Duration};

use y2m_client_core::{ClientRuntime, IncomingServerPacket};
use y2m_common::EventType;

use crate::{
    cli::{CommandArgs, FileArgs, JsonArgs, SendArgs, SendCommand, TextArgs},
    file_flow::send_file_flow,
    state::ConsoleState,
    util::{load_or_default_config, parse_json_value, resolve_config_path},
};

pub(crate) async fn run_send(args: SendArgs) -> anyhow::Result<()> {
    match args.kind {
        SendCommand::Text(text) => run_send_text(args.config, text).await,
        SendCommand::Json(json) => run_send_json(args.config, json).await,
        SendCommand::Command(command) => run_send_command(args.config, command).await,
        SendCommand::File(file) => run_send_file(args.config, file).await,
    }
}

async fn run_send_text(config: Option<std::path::PathBuf>, args: TextArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(config);
    let config = load_or_default_config(&config_path)?;
    let (runtime, _state) = crate::connect_with_console_plugin(config, None).await?;
    runtime.send_text(args.group.clone(), args.to.clone(), args.content.clone())?;
    tokio::time::sleep(Duration::from_millis(150)).await;
    let group = args.group.unwrap_or_else(|| runtime.identity().group_name.clone());
    let target = args.to.unwrap_or_else(|| "*".to_string());
    println!("已发送到 [{group}][{target}]");
    Ok(())
}

async fn run_send_json(config: Option<std::path::PathBuf>, args: JsonArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(config);
    let config = load_or_default_config(&config_path)?;
    let (runtime, _state) = crate::connect_with_console_plugin(config, None).await?;
    let content = parse_json_value(&args.content)?;
    runtime.send_json(args.group.clone(), args.to.clone(), content)?;
    tokio::time::sleep(Duration::from_millis(150)).await;
    let group = args.group.unwrap_or_else(|| runtime.identity().group_name.clone());
    let target = args.to.unwrap_or_else(|| "*".to_string());
    println!("已发送 JSON 到 [{group}][{target}]");
    Ok(())
}

async fn run_send_command(config: Option<std::path::PathBuf>, args: CommandArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(config);
    let config = load_or_default_config(&config_path)?;
    let wait_secs = args
        .timeout
        .or(config.command_wait_timeout_sec)
        .unwrap_or(30)
        .max(1);
    let (mut runtime, state) = crate::connect_with_console_plugin(config, None).await?;
    runtime.send_command(args.group.clone(), args.to.clone(), args.command.clone(), Some(wait_secs))?;
    let group = args.group.unwrap_or_else(|| runtime.identity().group_name.clone());
    let target = args.to.unwrap_or_else(|| "*".to_string());
    println!("已发送命令到 [{group}][{target}]");
    wait_for_command_result(&mut runtime, &state, wait_secs).await?;
    Ok(())
}

async fn run_send_file(config: Option<std::path::PathBuf>, args: FileArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(config);
    let config = load_or_default_config(&config_path)?;
    let (mut runtime, _state) = crate::connect_with_console_plugin(config, None).await?;
    send_file_flow(&mut runtime, &args.path, args.group, args.to, args.timeout).await
}

pub(crate) async fn wait_for_command_result(
    runtime: &mut ClientRuntime,
    console_state: &Arc<ConsoleState>,
    timeout_secs: u64,
) -> anyhow::Result<()> {
    let deadline = tokio::time::sleep(Duration::from_secs(timeout_secs.max(1) + 2));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => {
                console_state.flush_all_pending_command_results();
                println!("等待命令执行结果超时");
                break;
            }
            maybe_packet = runtime.recv_next_packet() => {
                let Some(packet) = maybe_packet else {
                    console_state.flush_all_pending_command_results();
                    break;
                };
                let is_result = matches!(
                    &packet,
                    IncomingServerPacket::Event(e) if e.payload.event_type == EventType::CommandResult
                );
                runtime.dispatch_packet(packet).await?;
                if is_result {
                    console_state.flush_all_pending_command_results();
                    break;
                }
            }
        }
    }
    Ok(())
}
