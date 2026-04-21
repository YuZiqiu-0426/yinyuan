use std::sync::Arc;

use tokio::sync::mpsc;
use y2m_client_core::{ClientRuntime, IncomingRuntimeMessage, IncomingServerPacket};

use crate::{
    cli::RunArgs,
    printer::cprintln,
    state::ConsoleState,
    types::SessionLoopExit,
    util::{load_or_default_config, resolve_config_path},
};

pub(crate) async fn run_run(args: RunArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(args.config);
    let config = load_or_default_config(&config_path)?;
    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
    crate::spawn_stdin_reader(line_tx);
    let mut console_state: Option<Arc<ConsoleState>> = None;
    let mut first_connection = true;

    loop {
        let (mut runtime, state) = crate::connect_with_console_plugin_with_retry(
            &config, console_state.clone(), args.reconnect_interval_sec,
        ).await?;
        console_state = Some(state.clone());
        if first_connection {
            cprintln!("已连接 [{} / {}]", runtime.identity().group_name, runtime.identity().client_name);
            print_console_control_help();
        } else {
            cprintln!("已重新连接 [{} / {}]", runtime.identity().group_name, runtime.identity().client_name);
            state.replay_after_reconnect();
        }
        let heartbeat = runtime.spawn_heartbeat_loop();
        let exit = run_run_session(&mut runtime, &state, &mut line_rx).await?;
        heartbeat.abort();
        match exit {
            SessionLoopExit::UserQuit => return Ok(()),
            SessionLoopExit::Disconnected if args.reconnect_interval_sec == 0 => {
                cprintln!("连接已断开");
                return Ok(());
            }
            SessionLoopExit::Disconnected => {
                cprintln!("连接已断开，将在 {} 秒后自动重连...", args.reconnect_interval_sec.max(1));
                first_connection = false;
            }
        }
    }
}

async fn run_run_session(
    runtime: &mut ClientRuntime,
    console_state: &Arc<ConsoleState>,
    line_rx: &mut mpsc::UnboundedReceiver<String>,
) -> anyhow::Result<SessionLoopExit> {
    let mut stdin_open = true;
    loop {
        tokio::select! {
            maybe_line = line_rx.recv(), if stdin_open => {
                match maybe_line {
                    Some(line) => handle_console_control_line(runtime, console_state, line)?,
                    None => stdin_open = false,
                }
            }
            maybe_message = runtime.recv_next_message() => {
                match maybe_message {
                    Some(IncomingRuntimeMessage::Packet(packet)) => {
                        if let IncomingServerPacket::Ack(ack) = &packet { console_state.handle_ack(ack); }
                        if let IncomingServerPacket::Error(e) = &packet { console_state.handle_server_error(e); }
                        runtime.dispatch_packet(packet).await?;
                    }
                    Some(IncomingRuntimeMessage::Binary(frame)) => {
                        console_state.handle_binary_frame(runtime, frame)?;
                    }
                    None => return Ok(SessionLoopExit::Disconnected),
                }
            }
        }
    }
}

fn handle_console_control_line(
    runtime: &ClientRuntime,
    console_state: &ConsoleState,
    line: String,
) -> anyhow::Result<()> {
    let line = line.trim();
    if line.is_empty() { return Ok(()); }
    if let Some(rest) = line.strip_prefix("/accept ") {
        handle_file_offer_decision(runtime, console_state, rest.trim(), true)?;
    } else if let Some(rest) = line.strip_prefix("/reject ") {
        handle_file_offer_decision(runtime, console_state, rest.trim(), false)?;
    } else if line == "/files" {
        console_state.print_file_queue();
    } else if let Some(rest) = line.strip_prefix("/abort ") {
        handle_file_abort(runtime, console_state, rest.trim())?;
    } else if line == "/help" {
        print_console_control_help();
    }
    Ok(())
}

pub(crate) fn handle_file_offer_decision(
    runtime: &ClientRuntime,
    console_state: &ConsoleState,
    file_id: &str,
    accept: bool,
) -> anyhow::Result<()> {
    let Ok(file_id) = uuid::Uuid::parse_str(file_id) else {
        cprintln!("fileId 格式错误: {}", file_id);
        return Ok(());
    };
    let handled = if accept {
        console_state.accept_pending_offer(runtime, file_id)?
    } else {
        console_state.reject_pending_offer(runtime, file_id)?
    };
    if !handled { cprintln!("未找到待处理文件: {}", file_id); }
    Ok(())
}

pub(crate) fn handle_file_abort(
    runtime: &ClientRuntime,
    console_state: &ConsoleState,
    file_id: &str,
) -> anyhow::Result<()> {
    let Ok(file_id) = uuid::Uuid::parse_str(file_id) else {
        cprintln!("fileId 格式错误: {}", file_id);
        return Ok(());
    };
    if !console_state.abort_transfer(runtime, file_id)? {
        cprintln!("未找到可取消的文件: {}", file_id);
    }
    Ok(())
}

fn print_console_control_help() {
    cprintln!("可用控制命令:");
    cprintln!("/files 查看本地文件状态");
    cprintln!("/accept <fileId> 接收待确认文件");
    cprintln!("/reject <fileId> 拒绝待确认文件");
    cprintln!("/abort <fileId> 取消发送或接收中的文件");
    cprintln!("/help 查看控制命令");
}
