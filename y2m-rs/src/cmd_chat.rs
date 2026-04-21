use std::sync::Arc;

use tokio::sync::mpsc;
use y2m_client_core::{ClientRuntime, IncomingRuntimeMessage, IncomingServerPacket};

use crate::{
    cli::ChatArgs,
    cmd_run::{handle_file_abort, handle_file_offer_decision},
    printer::cprintln,
    state::ConsoleState,
    types::SessionLoopExit,
    util::{load_or_default_config, parse_json_value, resolve_config_path},
};

pub(crate) async fn run_chat(args: ChatArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(args.config);
    let config = load_or_default_config(&config_path)?;
    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
    let printer = crate::line_editor::spawn_line_editor(line_tx);
    crate::printer::install(printer);
    let mut session_group = args.group;
    let mut session_client = args.to;
    let mut console_state: Option<Arc<ConsoleState>> = None;
    let mut first_connection = true;

    loop {
        let (mut runtime, state) = crate::connect_with_console_plugin_with_retry(
            &config, console_state.clone(), args.reconnect_interval_sec,
        ).await?;
        console_state = Some(state.clone());
        runtime.session_mut().group_name = session_group.clone();
        runtime.session_mut().client_name = session_client.clone();
        if first_connection {
            print_chat_status(&runtime);
            print_chat_help();
        } else {
            cprintln!("已重新连接 [{} / {}]", runtime.identity().group_name, runtime.identity().client_name);
            state.replay_after_reconnect();
            print_chat_status(&runtime);
        }
        let heartbeat = runtime.spawn_heartbeat_loop();
        let exit = run_chat_session(&mut runtime, &state, &mut line_rx).await?;
        heartbeat.abort();
        session_group = runtime.session().group_name.clone();
        session_client = runtime.session().client_name.clone();
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

async fn run_chat_session(
    runtime: &mut ClientRuntime,
    console_state: &Arc<ConsoleState>,
    line_rx: &mut mpsc::UnboundedReceiver<String>,
) -> anyhow::Result<SessionLoopExit> {
    loop {
        tokio::select! {
            maybe_line = line_rx.recv() => {
                match maybe_line {
                    Some(line) => {
                        if !handle_chat_line(runtime, console_state, line).await? {
                            return Ok(SessionLoopExit::UserQuit);
                        }
                    }
                    None => return Ok(SessionLoopExit::UserQuit),
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

async fn handle_chat_line(
    runtime: &mut ClientRuntime,
    console_state: &Arc<ConsoleState>,
    line: String,
) -> anyhow::Result<bool> {
    let line = line.trim();
    if line.is_empty() { return Ok(true); }
    if let Some(keep_going) = handle_chat_slash_command(runtime, console_state, line).await? {
        return Ok(keep_going);
    }
    let group_name = runtime.session().group_name.clone();
    let client_name = runtime.session().client_name.clone();
    runtime.send_text(group_name, client_name, line)?;
    Ok(true)
}

async fn handle_chat_slash_command(
    runtime: &mut ClientRuntime,
    console_state: &Arc<ConsoleState>,
    line: &str,
) -> anyhow::Result<Option<bool>> {
    if let Some(rest) = line.strip_prefix("/to ") {
        runtime.session_mut().client_name = Some(rest.trim().to_string());
        print_chat_status(runtime);
        return Ok(Some(true));
    }
    if let Some(rest) = line.strip_prefix("/group ") {
        runtime.session_mut().group_name = Some(rest.trim().to_string());
        print_chat_status(runtime);
        return Ok(Some(true));
    }
    if let Some(rest) = line.strip_prefix("/json ") {
        match parse_json_value(rest.trim()) {
            Ok(content) => {
                let g = runtime.session().group_name.clone();
                let c = runtime.session().client_name.clone();
                runtime.send_json(g, c, content)?;
            }
            Err(e) => cprintln!("JSON 格式错误: {e}"),
        }
        return Ok(Some(true));
    }
    if let Some(keep) = handle_chat_file_command(runtime, console_state, line)? {
        return Ok(Some(keep));
    }
    if let Some(keep) = handle_chat_command_cmd(runtime, line)? {
        return Ok(Some(keep));
    }
    match line {
        "/to" => { runtime.session_mut().client_name = None; print_chat_status(runtime); Ok(Some(true)) }
        "/group" => { runtime.session_mut().group_name = None; print_chat_status(runtime); Ok(Some(true)) }
        "/status" => { print_chat_status(runtime); Ok(Some(true)) }
        "/help" => { print_chat_help(); Ok(Some(true)) }
        "/exit" => Ok(Some(false)),
        _ => Ok(None),
    }
}

fn handle_chat_file_command(
    runtime: &mut ClientRuntime,
    console_state: &Arc<ConsoleState>,
    line: &str,
) -> anyhow::Result<Option<bool>> {
    if let Some(rest) = line.strip_prefix("/file ") {
        let path = std::path::PathBuf::from(rest.trim());
        let g = runtime.session().group_name.clone();
        let c = runtime.session().client_name.clone();
        console_state.start_outgoing_file(runtime, &path, g, c)?;
        return Ok(Some(true));
    }
    if let Some(rest) = line.strip_prefix("/accept ") {
        handle_file_offer_decision(runtime, console_state, rest.trim(), true)?;
        return Ok(Some(true));
    }
    if let Some(rest) = line.strip_prefix("/reject ") {
        handle_file_offer_decision(runtime, console_state, rest.trim(), false)?;
        return Ok(Some(true));
    }
    if line == "/files" {
        console_state.print_file_queue();
        return Ok(Some(true));
    }
    if let Some(rest) = line.strip_prefix("/abort ") {
        handle_file_abort(runtime, console_state, rest.trim())?;
        return Ok(Some(true));
    }
    Ok(None)
}

fn handle_chat_command_cmd(runtime: &mut ClientRuntime, line: &str) -> anyhow::Result<Option<bool>> {
    if let Some(rest) = line.strip_prefix("/command ") {
        let command = rest.trim();
        if command.is_empty() { cprintln!("请提供要执行的命令"); return Ok(Some(true)); }
        let g = runtime.session().group_name.clone();
        let c = runtime.session().client_name.clone();
        runtime.send_command(g, c, command, Some(30))?;
        return Ok(Some(true));
    }
    Ok(None)
}

pub(crate) fn print_chat_status(runtime: &ClientRuntime) {
    let group = runtime.session().group_name.clone()
        .unwrap_or_else(|| runtime.identity().group_name.clone());
    let target = runtime.session().client_name.clone().unwrap_or_else(|| "*".to_string());
    cprintln!("当前会话: group={group}, to={target}");
}

fn print_chat_help() {
    cprintln!("/to <client> 切换目标用户");
    cprintln!("/to 清空目标用户并恢复广播");
    cprintln!("/group <group> 切换目标分组");
    cprintln!("/group 清空目标分组并恢复默认分组");
    cprintln!("/json <json> 发送 JSON 消息");
    cprintln!("/command <cmd> 发送命令请求");
    cprintln!("/file <path> 发送文件");
    cprintln!("/files 查看本地文件状态");
    cprintln!("/accept <fileId> 接收待确认文件");
    cprintln!("/reject <fileId> 拒绝待确认文件");
    cprintln!("/abort <fileId> 取消发送或接收中的文件");
    cprintln!("/status 查看当前会话");
    cprintln!("/help 查看帮助");
    cprintln!("/exit 退出会话");
}
