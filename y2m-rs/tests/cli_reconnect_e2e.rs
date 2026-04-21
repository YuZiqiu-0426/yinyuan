use std::{fs, path::Path, time::Duration};

mod support;

use y2m_client_core::{build_file_offer_event_packet, IncomingServerPacket};
use y2m_common::{BinaryChunkHeader, EventType};

use support::connect_runtime;
use support::cli::{
    create_temp_dir, init_client_config, reserve_server_addr, run_y2m_checked,
    spawn_server_process_at, spawn_y2m, workspace_root, parse_file_offer_line,
};

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_run_reconnects_after_server_restart() -> anyhow::Result<()> {
    let addr = reserve_server_addr()?;
    let server_url = format!("ws://{addr}/ws");
    let mut server = spawn_server_process_at(addr)?;
    let temp_dir = create_temp_dir("run-reconnect")?;
    let alice_config = temp_dir.join("alice.json");
    let bob_config = temp_dir.join("bob.json");

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;
    init_client_config(&bob_config, &server_url, "group1", "bob", None)?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["run", "--config", bob_config_text.as_str(), "--reconnect-interval-sec", "1"],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("已连接 [group1 / bob]", Duration::from_secs(10))?;

    server.kill();
    bob.wait_for_contains("连接已断开，将在 1 秒后自动重连...", Duration::from_secs(10))?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let _server2 = spawn_server_process_at(addr)?;
    bob.wait_for_contains("已重新连接 [group1 / bob]", Duration::from_secs(15))?;

    let alice_config_text = alice_config.to_string_lossy().to_string();
    let text_output = run_y2m_checked(
        &["send", "--config", alice_config_text.as_str(), "text", "--to", "bob", "hello after reconnect"],
        &workspace_root(),
        &[],
    )?;
    assert!(text_output.contains("已发送到 [group1][bob]"));
    bob.wait_for_contains("[group1][alice] hello after reconnect", Duration::from_secs(10))?;

    Ok(())
}

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_reconnect_replays_failed_inflight_file_state() -> anyhow::Result<()> {
    let addr = reserve_server_addr()?;
    let server_url = format!("ws://{addr}/ws");
    let mut server = spawn_server_process_at(addr)?;
    let temp_dir = create_temp_dir("chat-reconnect-file-state")?;
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&bob_config, &server_url, "group1", "bob", Some(&bob_download_dir))?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["chat", "--config", bob_config_text.as_str(), "--reconnect-interval-sec", "1"],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let file_id = uuid::Uuid::new_v4();
    let payload = vec![b'x'; 1024];
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "reconnect.bin", (payload.len() * 2) as u64, "application/octet-stream",
        "pending-reconnect-sha", payload.len() as u32, 2,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let first_chunk = BinaryChunkHeader::new(file_id, 0, 2, payload.len() as u32)
        .encode_with_payload(&payload);
    alice_runtime.connection().send_binary(first_chunk)?;
    bob.wait_for_contains("接收进度:", Duration::from_secs(10))?;

    server.kill();
    bob.wait_for_contains("连接已断开，将在 1 秒后自动重连...", Duration::from_secs(10))?;

    let _server2 = spawn_server_process_at(addr)?;
    bob.wait_for_contains("已重新连接 [group1 / bob]", Duration::from_secs(15))?;
    bob.wait_for_contains("接收失败: id=", Duration::from_secs(10))?;
    bob.wait_for_contains("reason=连接中断，请等待对方在线后重新发送", Duration::from_secs(10))?;

    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;
    assert!(!Path::new(&bob_download_dir.join("reconnect.bin")).exists());

    Ok(())
}

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_reconnect_replays_failed_pending_offer() -> anyhow::Result<()> {
    let addr = reserve_server_addr()?;
    let server_url = format!("ws://{addr}/ws");
    let mut server = spawn_server_process_at(addr)?;
    let temp_dir = create_temp_dir("chat-reconnect-pending-offer")?;
    let bob_config = temp_dir.join("bob.json");

    init_client_config(&bob_config, &server_url, "group1", "bob", None)?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["chat", "--config", bob_config_text.as_str(), "--reconnect-interval-sec", "1"],
        &temp_dir,
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let file_id = uuid::Uuid::new_v4();
    let payload = vec![b'x'; 1024];
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "pending-offer.bin", payload.len() as u64, "application/octet-stream",
        "pending-offer-sha", payload.len() as u32, 1,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());

    server.kill();
    bob.wait_for_contains("连接已断开，将在 1 秒后自动重连...", Duration::from_secs(10))?;

    let _server2 = spawn_server_process_at(addr)?;
    bob.wait_for_contains("已重新连接 [group1 / bob]", Duration::from_secs(15))?;
    bob.wait_for_contains("待确认文件已失效: id=", Duration::from_secs(10))?;
    bob.wait_for_contains("reason=连接中断，请等待对方在线后重新发送", Duration::from_secs(10))?;

    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;
    assert!(!Path::new(&temp_dir.join("downloads").join("pending-offer.bin")).exists());

    Ok(())
}

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_reconnect_replays_failed_outgoing_waiting_accept() -> anyhow::Result<()> {
    let addr = reserve_server_addr()?;
    let server_url = format!("ws://{addr}/ws");
    let mut server = spawn_server_process_at(addr)?;
    let temp_dir = create_temp_dir("chat-reconnect-outgoing-waiting-accept")?;
    let alice_config = temp_dir.join("alice.json");
    let sample_path = temp_dir.join("outgoing-pending.bin");
    fs::write(&sample_path, b"hello reconnect outgoing")?;

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;

    let alice_config_text = alice_config.to_string_lossy().to_string();
    let mut alice = spawn_y2m(
        &["chat", "--config", alice_config_text.as_str(), "--to", "bob", "--reconnect-interval-sec", "1"],
        &workspace_root(),
        &[],
    )?;
    alice.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let mut bob_runtime = connect_runtime(server_url.clone(), "group1", "bob", vec![]).await?;
    alice.write_line(&format!("/file {}", sample_path.display()))?;

    loop {
        match bob_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) if event.payload.event_type == EventType::FileOffer => break,
            _ => {}
        }
    }

    alice.wait_for_contains("已发送文件申请，等待对方确认...", Duration::from_secs(10))?;
    alice.write_line("/files")?;
    alice.wait_for_contains("外发文件:", Duration::from_secs(10))?;
    alice.wait_for_contains("状态=等待对方接受", Duration::from_secs(10))?;

    server.kill();
    alice.wait_for_contains("连接已断开，将在 1 秒后自动重连...", Duration::from_secs(10))?;

    let _server2 = spawn_server_process_at(addr)?;
    alice.wait_for_contains("已重新连接 [group1 / alice]", Duration::from_secs(15))?;
    alice.wait_for_contains("发送失败: id=", Duration::from_secs(10))?;
    alice.wait_for_contains("reason=连接中断，请等待对方在线后重新发送", Duration::from_secs(10))?;

    alice.write_line("/files")?;
    alice.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;

    Ok(())
}
