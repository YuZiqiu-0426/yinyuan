use std::{fs, path::Path, time::Duration};

mod support;

use y2m_client_core::{
    build_file_complete_event_packet, build_file_offer_event_packet, IncomingServerPacket,
};
use y2m_common::{AckStatus, BinaryChunkHeader, ErrorCode, EventType};

use support::connect_runtime;
use support::cli::{
    create_temp_dir, init_client_config, reserve_server_addr, run_y2m_checked,
    spawn_server_process, spawn_server_process_at, spawn_y2m, workspace_root,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_text_json_and_command_success_end_to_end() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("text-json-command")?;
    let alice_text_config = temp_dir.join("alice-text.json");
    let alice_json_config = temp_dir.join("alice-json.json");
    let alice_command_config = temp_dir.join("alice-command.json");
    let bob_config = temp_dir.join("bob.json");

    init_client_config(&alice_text_config, &server_url, "group1", "alice-text", None)?;
    init_client_config(&alice_json_config, &server_url, "group1", "alice-json", None)?;
    init_client_config(
        &alice_command_config,
        &server_url,
        "group1",
        "alice-command",
        None,
    )?;
    init_client_config(&bob_config, &server_url, "group1", "bob", None)?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["run", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("已连接 [group1 / bob]", Duration::from_secs(10))?;

    let alice_text_config_text = alice_text_config.to_string_lossy().to_string();
    let text_output = run_y2m_checked(
        &[
            "send",
            "--config",
            alice_text_config_text.as_str(),
            "text",
            "--to",
            "bob",
            "hello from cli",
        ],
        &workspace_root(),
        &[],
    )?;
    assert!(text_output.contains("已发送到 [group1][bob]"));
    bob.wait_for_contains("[group1][alice-text] hello from cli", Duration::from_secs(10))?;

    let alice_json_config_text = alice_json_config.to_string_lossy().to_string();
    let json_output = run_y2m_checked(
        &[
            "send",
            "--config",
            alice_json_config_text.as_str(),
            "json",
            "--to",
            "bob",
            r#"{"message":"hello json cli"}"#,
        ],
        &workspace_root(),
        &[],
    )?;
    assert!(json_output.contains("已发送 JSON 到 [group1][bob]"));
    bob.wait_for_contains(
        r#"[group1][alice-json] {"message":"hello json cli"}"#,
        Duration::from_secs(10),
    )?;

    let alice_command_config_text = alice_command_config.to_string_lossy().to_string();
    let command_output = run_y2m_checked(
        &[
            "send",
            "--config",
            alice_command_config_text.as_str(),
            "command",
            "--to",
            "bob",
            "echo hello from cli command",
        ],
        &workspace_root(),
        &[],
    )?;
    assert!(command_output.contains("已发送命令到 [group1][bob]"));
    assert!(command_output.contains("command_result"));
    assert!(command_output.contains(r#""exitCode":0"#));
    assert!(command_output.contains("hello from cli command"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_command_timeout_returns_command_result() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("command-timeout")?;
    let alice_config = temp_dir.join("alice.json");
    let bob_config = temp_dir.join("bob.json");

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;
    init_client_config(&bob_config, &server_url, "group1", "bob", None)?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["run", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("已连接 [group1 / bob]", Duration::from_secs(10))?;

    let alice_config_text = alice_config.to_string_lossy().to_string();
    let command_output = run_y2m_checked(
        &[
            "send",
            "--config",
            alice_config_text.as_str(),
            "command",
            "--to",
            "bob",
            "--timeout",
            "1",
            timeout_command().as_str(),
        ],
        &workspace_root(),
        &[],
    )?;
    assert!(command_output.contains("已发送命令到 [group1][bob]"));
    assert!(command_output.contains("command_result"));
    assert!(command_output.contains(r#""exitCode":-1"#));
    assert!(command_output.contains("command timed out after 1s"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_command_failure_returns_command_result() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("command-failure")?;
    let alice_config = temp_dir.join("alice.json");
    let bob_config = temp_dir.join("bob.json");

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;
    init_client_config(&bob_config, &server_url, "group1", "bob", None)?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["run", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("已连接 [group1 / bob]", Duration::from_secs(10))?;

    let alice_config_text = alice_config.to_string_lossy().to_string();
    let missing_command = missing_command_name();
    let command_output = run_y2m_checked(
        &[
            "send",
            "--config",
            alice_config_text.as_str(),
            "command",
            "--to",
            "bob",
            missing_command,
        ],
        &workspace_root(),
        &[],
    )?;
    assert!(command_output.contains("已发送命令到 [group1][bob]"));
    assert!(command_output.contains("command_result"));
    assert!(command_output.contains(&format!(r#""exitCode":{}"#, missing_command_exit_code())));
    assert!(command_output.contains(missing_command));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_file_accept_end_to_end() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-accept")?;
    let alice_config = temp_dir.join("alice.json");
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;
    init_client_config(
        &bob_config,
        &server_url,
        "group1",
        "bob",
        Some(&bob_download_dir),
    )?;

    let alice_config_text = alice_config.to_string_lossy().to_string();
    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut alice = spawn_y2m(
        &["chat", "--config", alice_config_text.as_str(), "--to", "bob"],
        &workspace_root(),
        &[],
    )?;
    let mut bob = spawn_y2m(
        &["chat", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    alice.wait_for_contains("当前会话:", Duration::from_secs(10))?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let sample_path = temp_dir.join("sample.txt");
    fs::write(&sample_path, b"hello file from cli chat")?;
    alice.write_line(&format!("/file {}", sample_path.display()))?;

    let file_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    bob.write_line("/files")?;
    bob.wait_for_contains("待确认文件:", Duration::from_secs(10))?;
    bob.wait_for_contains("状态=待确认", Duration::from_secs(10))?;
    bob.wait_for_contains(&file_id, Duration::from_secs(10))?;
    bob.write_line(&format!("/accept {file_id}"))?;
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;
    alice.wait_for_contains("对方已接受", Duration::from_secs(10))?;
    bob.wait_for_contains("文件已保存:", Duration::from_secs(10))?;
    alice.wait_for_contains("文件发送完成:", Duration::from_secs(10))?;

    let saved_file = bob_download_dir.join("sample.txt");
    let saved = fs::read(&saved_file)?;
    assert_eq!(saved, b"hello file from cli chat");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_send_file_reject_in_chat_end_to_end() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-reject")?;
    let alice_config = temp_dir.join("alice.json");
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;
    init_client_config(
        &bob_config,
        &server_url,
        "group1",
        "bob",
        Some(&bob_download_dir),
    )?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["chat", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let sample_path = temp_dir.join("reject.txt");
    fs::write(&sample_path, b"reject me")?;
    let alice_config_text = alice_config.to_string_lossy().to_string();
    let sample_text = sample_path.to_string_lossy().to_string();
    let mut alice = spawn_y2m(
        &[
            "send",
            "--config",
            alice_config_text.as_str(),
            "file",
            "--to",
            "bob",
            sample_text.as_str(),
        ],
        &workspace_root(),
        &[],
    )?;

    let file_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    bob.write_line(&format!("/reject {file_id}"))?;
    bob.wait_for_contains("已拒绝文件:", Duration::from_secs(10))?;
    alice.wait_for_contains("对方拒绝接收文件: rejected by user", Duration::from_secs(10))?;

    let status = alice.wait()?;
    assert!(!status.success(), "send file should fail when target rejects");
    assert!(!Path::new(&bob_download_dir.join("reject.txt")).exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_file_abort_end_to_end() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-abort")?;
    let alice_config = temp_dir.join("alice.json");
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;
    init_client_config(
        &bob_config,
        &server_url,
        "group1",
        "bob",
        Some(&bob_download_dir),
    )?;

    let alice_config_text = alice_config.to_string_lossy().to_string();
    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut alice = spawn_y2m(
        &["chat", "--config", alice_config_text.as_str(), "--to", "bob"],
        &workspace_root(),
        &[],
    )?;
    let mut bob = spawn_y2m(
        &["chat", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    alice.wait_for_contains("当前会话:", Duration::from_secs(10))?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let sample_path = temp_dir.join("abort.bin");
    fs::write(&sample_path, vec![b'x'; 32 * 1024 * 1024])?;
    alice.write_line(&format!("/file {}", sample_path.display()))?;

    let file_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    bob.write_line(&format!("/accept {file_id}"))?;
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;
    bob.wait_for_contains("接收进度:", Duration::from_secs(10))?;
    bob.write_line(&format!("/abort {file_id}"))?;

    bob.wait_for_contains("已取消接收:", Duration::from_secs(10))?;
    alice.wait_for_contains("发送已取消:", Duration::from_secs(10))?;
    assert!(!Path::new(&bob_download_dir.join("abort.bin")).exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_file_checksum_failure_cleans_local_state() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-checksum-failure")?;
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&bob_config, &server_url, "group1", "bob", Some(&bob_download_dir))?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["chat", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let payload = b"hello checksum failure".to_vec();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        file_id,
        "broken.txt",
        payload.len() as u64,
        "text/plain",
        "deadbeef",
        payload.len() as u32,
        1,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());
    bob.write_line(&format!("/accept {offered_id}"))?;
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) => {
                if event.payload.event_type == EventType::FileAccept {
                    break;
                }
            }
            _ => {}
        }
    }

    let chunk = BinaryChunkHeader::new(file_id, 0, 1, payload.len() as u32)
        .encode_with_payload(&payload);
    alice_runtime.connection().send_binary(chunk)?;
    bob.wait_for_contains("接收进度:", Duration::from_secs(10))?;

    let complete_packet = build_file_complete_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        file_id,
        payload.len() as u64,
        "badbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadb",
    );
    let complete_request_id = complete_packet.request_id.clone();
    alice_runtime.connection().send_json_packet(&complete_packet)?;

    bob.wait_for_contains("文件校验失败:", Duration::from_secs(10))?;
    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Ack(packet) => {
                if packet.request_id == complete_request_id {
                    assert_eq!(packet.payload.status, AckStatus::Rejected);
                    break;
                }
            }
            _ => {}
        }
    }

    assert!(!Path::new(&bob_download_dir.join("broken.txt")).exists());

    let extra_chunk = BinaryChunkHeader::new(file_id, 0, 1, payload.len() as u32)
        .encode_with_payload(&payload);
    alice_runtime.connection().send_binary(extra_chunk)?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Error(packet) => {
                assert_eq!(packet.payload.code, ErrorCode::FileTransferNotAccepted);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_out_of_order_chunk_fails_and_aborts_transfer() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-out-of-order")?;
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&bob_config, &server_url, "group1", "bob", Some(&bob_download_dir))?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["chat", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let first_payload = b"chunk-one".to_vec();
    let second_payload = b"chunk-two".to_vec();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        file_id,
        "out-of-order.txt",
        (first_payload.len() + second_payload.len()) as u64,
        "text/plain",
        "out-of-order-sha",
        first_payload.len() as u32,
        2,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());
    bob.write_line(&format!("/accept {offered_id}"))?;
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) => {
                if event.payload.event_type == EventType::FileAccept {
                    break;
                }
            }
            _ => {}
        }
    }

    let second_chunk = BinaryChunkHeader::new(file_id, 1, 2, second_payload.len() as u32)
        .encode_with_payload(&second_payload);
    alice_runtime.connection().send_binary(second_chunk)?;

    bob.wait_for_contains("文件接收失败:", Duration::from_secs(10))?;
    bob.wait_for_contains("chunk sequence mismatch: expected 0, got 1", Duration::from_secs(10))?;
    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) => {
                if event.payload.event_type == EventType::FileAbort {
                    assert_eq!(
                        event.payload.metadata.get("reason").and_then(|value| value.as_str()),
                        Some("chunk sequence mismatch: expected 0, got 1")
                    );
                    break;
                }
            }
            _ => {}
        }
    }

    assert!(!Path::new(&bob_download_dir.join("out-of-order.txt")).exists());

    let first_chunk = BinaryChunkHeader::new(file_id, 0, 2, first_payload.len() as u32)
        .encode_with_payload(&first_payload);
    alice_runtime.connection().send_binary(first_chunk)?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Error(packet) => {
                assert_eq!(packet.payload.code, ErrorCode::FileTransferNotAccepted);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_duplicate_chunk_fails_and_aborts_transfer() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-duplicate-chunk")?;
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&bob_config, &server_url, "group1", "bob", Some(&bob_download_dir))?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["chat", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let first_payload = b"chunk-one".to_vec();
    let second_payload = b"chunk-two".to_vec();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        file_id,
        "duplicate-chunk.txt",
        (first_payload.len() + second_payload.len()) as u64,
        "text/plain",
        "duplicate-chunk-sha",
        first_payload.len() as u32,
        2,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());
    bob.write_line(&format!("/accept {offered_id}"))?;
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) => {
                if event.payload.event_type == EventType::FileAccept {
                    break;
                }
            }
            _ => {}
        }
    }

    let first_chunk = BinaryChunkHeader::new(file_id, 0, 2, first_payload.len() as u32)
        .encode_with_payload(&first_payload);
    alice_runtime.connection().send_binary(first_chunk.clone())?;
    bob.wait_for_contains("接收进度:", Duration::from_secs(10))?;

    alice_runtime.connection().send_binary(first_chunk)?;

    bob.wait_for_contains("文件接收失败:", Duration::from_secs(10))?;
    bob.wait_for_contains("chunk sequence mismatch: expected 1, got 0", Duration::from_secs(10))?;
    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) => {
                if event.payload.event_type == EventType::FileAbort {
                    assert_eq!(
                        event.payload.metadata.get("reason").and_then(|value| value.as_str()),
                        Some("chunk sequence mismatch: expected 1, got 0")
                    );
                    break;
                }
            }
            _ => {}
        }
    }

    assert!(!Path::new(&bob_download_dir.join("duplicate-chunk.txt")).exists());

    let second_chunk = BinaryChunkHeader::new(file_id, 1, 2, second_payload.len() as u32)
        .encode_with_payload(&second_payload);
    alice_runtime.connection().send_binary(second_chunk)?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Error(packet) => {
                assert_eq!(packet.payload.code, ErrorCode::FileTransferNotAccepted);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_total_chunks_mismatch_fails_and_aborts_transfer() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-total-chunks-mismatch")?;
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&bob_config, &server_url, "group1", "bob", Some(&bob_download_dir))?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["chat", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let first_payload = b"chunk-one".to_vec();
    let second_payload = b"chunk-two".to_vec();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        file_id,
        "total-chunks-mismatch.txt",
        (first_payload.len() + second_payload.len()) as u64,
        "text/plain",
        "total-chunks-mismatch-sha",
        first_payload.len() as u32,
        2,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());
    bob.write_line(&format!("/accept {offered_id}"))?;
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) => {
                if event.payload.event_type == EventType::FileAccept {
                    break;
                }
            }
            _ => {}
        }
    }

    let bad_chunk = BinaryChunkHeader::new(file_id, 0, 3, first_payload.len() as u32)
        .encode_with_payload(&first_payload);
    alice_runtime.connection().send_binary(bad_chunk)?;

    bob.wait_for_contains("文件接收失败:", Duration::from_secs(10))?;
    bob.wait_for_contains("chunk total mismatch: expected 2, got 3", Duration::from_secs(10))?;
    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) => {
                if event.payload.event_type == EventType::FileAbort {
                    assert_eq!(
                        event.payload.metadata.get("reason").and_then(|value| value.as_str()),
                        Some("chunk total mismatch: expected 2, got 3")
                    );
                    break;
                }
            }
            _ => {}
        }
    }

    assert!(!Path::new(&bob_download_dir.join("total-chunks-mismatch.txt")).exists());

    let valid_chunk = BinaryChunkHeader::new(file_id, 0, 2, first_payload.len() as u32)
        .encode_with_payload(&first_payload);
    alice_runtime.connection().send_binary(valid_chunk)?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Error(packet) => {
                assert_eq!(packet.payload.code, ErrorCode::FileTransferNotAccepted);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_file_size_mismatch_cleans_local_state() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-size-mismatch")?;
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&bob_config, &server_url, "group1", "bob", Some(&bob_download_dir))?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["chat", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let payload = b"hello size mismatch".to_vec();
    let declared_size = payload.len() as u64 + 5;
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        file_id,
        "size-mismatch.txt",
        declared_size,
        "text/plain",
        "d5604a22cf90cf0cc4855ac44474f30129a5b894face69e18ce54d56fedf4448",
        payload.len() as u32,
        1,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());
    bob.write_line(&format!("/accept {offered_id}"))?;
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) => {
                if event.payload.event_type == EventType::FileAccept {
                    break;
                }
            }
            _ => {}
        }
    }

    let chunk = BinaryChunkHeader::new(file_id, 0, 1, payload.len() as u32)
        .encode_with_payload(&payload);
    alice_runtime.connection().send_binary(chunk)?;
    bob.wait_for_contains("接收进度:", Duration::from_secs(10))?;

    let complete_packet = build_file_complete_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        file_id,
        declared_size,
        "d5604a22cf90cf0cc4855ac44474f30129a5b894face69e18ce54d56fedf4448",
    );
    let complete_request_id = complete_packet.request_id.clone();
    alice_runtime.connection().send_json_packet(&complete_packet)?;

    bob.wait_for_contains("文件校验失败:", Duration::from_secs(10))?;
    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Ack(packet) => {
                if packet.request_id == complete_request_id {
                    assert_eq!(packet.payload.status, AckStatus::Rejected);
                    break;
                }
            }
            _ => {}
        }
    }

    assert!(!Path::new(&bob_download_dir.join("size-mismatch.txt")).exists());

    let extra_chunk = BinaryChunkHeader::new(file_id, 0, 1, payload.len() as u32)
        .encode_with_payload(&payload);
    alice_runtime.connection().send_binary(extra_chunk)?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Error(packet) => {
                assert_eq!(packet.payload.code, ErrorCode::FileTransferNotAccepted);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_missing_chunk_on_complete_is_rejected() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-missing-chunk")?;
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&bob_config, &server_url, "group1", "bob", Some(&bob_download_dir))?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &["chat", "--config", bob_config_text.as_str()],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let first_payload = b"chunk-one".to_vec();
    let second_payload = b"chunk-two".to_vec();
    let declared_size = (first_payload.len() + second_payload.len()) as u64;
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        file_id,
        "missing-chunk.txt",
        declared_size,
        "text/plain",
        "missing-chunk-sha",
        first_payload.len() as u32,
        2,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());
    bob.write_line(&format!("/accept {offered_id}"))?;
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) => {
                if event.payload.event_type == EventType::FileAccept {
                    break;
                }
            }
            _ => {}
        }
    }

    let first_chunk = BinaryChunkHeader::new(file_id, 0, 2, first_payload.len() as u32)
        .encode_with_payload(&first_payload);
    alice_runtime.connection().send_binary(first_chunk)?;
    bob.wait_for_contains("接收进度:", Duration::from_secs(10))?;

    let complete_packet = build_file_complete_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        file_id,
        declared_size,
        "missing-chunk-sha",
    );
    let complete_request_id = complete_packet.request_id.clone();
    alice_runtime.connection().send_json_packet(&complete_packet)?;

    bob.wait_for_contains("文件接收失败:", Duration::from_secs(10))?;
    bob.wait_for_contains("reason=分片不完整", Duration::from_secs(10))?;
    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Ack(packet) => {
                if packet.request_id == complete_request_id {
                    assert_eq!(packet.payload.status, AckStatus::Rejected);
                    break;
                }
            }
            _ => {}
        }
    }

    assert!(!Path::new(&bob_download_dir.join("missing-chunk.txt")).exists());

    let second_chunk = BinaryChunkHeader::new(file_id, 1, 2, second_payload.len() as u32)
        .encode_with_payload(&second_payload);
    alice_runtime.connection().send_binary(second_chunk)?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Error(packet) => {
                assert_eq!(packet.payload.code, ErrorCode::FileTransferNotAccepted);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

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
        &[
            "run",
            "--config",
            bob_config_text.as_str(),
            "--reconnect-interval-sec",
            "1",
        ],
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
        &[
            "send",
            "--config",
            alice_config_text.as_str(),
            "text",
            "--to",
            "bob",
            "hello after reconnect",
        ],
        &workspace_root(),
        &[],
    )?;
    assert!(text_output.contains("已发送到 [group1][bob]"));
    bob.wait_for_contains("[group1][alice] hello after reconnect", Duration::from_secs(10))?;

    Ok(())
}

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
        &[
            "chat",
            "--config",
            bob_config_text.as_str(),
            "--reconnect-interval-sec",
            "1",
        ],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let file_id = uuid::Uuid::new_v4();
    let payload = vec![b'x'; 1024];
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        file_id,
        "reconnect.bin",
        (payload.len() * 2) as u64,
        "application/octet-stream",
        "pending-reconnect-sha",
        payload.len() as u32,
        2,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());
    bob.write_line(&format!("/accept {offered_id}"))?;
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
    bob.wait_for_contains(
        "接收失败: id=",
        Duration::from_secs(10),
    )?;
    bob.wait_for_contains(
        "reason=连接中断，请等待对方在线后重新发送",
        Duration::from_secs(10),
    )?;

    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;
    assert!(!Path::new(&bob_download_dir.join("reconnect.bin")).exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_chat_reconnect_replays_failed_pending_offer() -> anyhow::Result<()> {
    let addr = reserve_server_addr()?;
    let server_url = format!("ws://{addr}/ws");
    let mut server = spawn_server_process_at(addr)?;
    let temp_dir = create_temp_dir("chat-reconnect-pending-offer")?;
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&bob_config, &server_url, "group1", "bob", Some(&bob_download_dir))?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(
        &[
            "chat",
            "--config",
            bob_config_text.as_str(),
            "--reconnect-interval-sec",
            "1",
        ],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let file_id = uuid::Uuid::new_v4();
    let payload = vec![b'x'; 1024];
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        file_id,
        "pending-offer.bin",
        payload.len() as u64,
        "application/octet-stream",
        "pending-offer-sha",
        payload.len() as u32,
        1,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());

    server.kill();
    bob.wait_for_contains("连接已断开，将在 1 秒后自动重连...", Duration::from_secs(10))?;

    let _server2 = spawn_server_process_at(addr)?;
    bob.wait_for_contains("已重新连接 [group1 / bob]", Duration::from_secs(15))?;
    bob.wait_for_contains("待确认文件已失效: id=", Duration::from_secs(10))?;
    bob.wait_for_contains(
        "reason=连接中断，请等待对方在线后重新发送",
        Duration::from_secs(10),
    )?;

    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;
    assert!(!Path::new(&bob_download_dir.join("pending-offer.bin")).exists());

    Ok(())
}

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
        &[
            "chat",
            "--config",
            alice_config_text.as_str(),
            "--to",
            "bob",
            "--reconnect-interval-sec",
            "1",
        ],
        &workspace_root(),
        &[],
    )?;
    alice.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let mut bob_runtime = connect_runtime(server_url.clone(), "group1", "bob", vec![]).await?;
    alice.write_line(&format!("/file {}", sample_path.display()))?;

    loop {
        match bob_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) => {
                if event.payload.event_type == EventType::FileOffer {
                    break;
                }
            }
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
    alice.wait_for_contains(
        "reason=连接中断，请等待对方在线后重新发送",
        Duration::from_secs(10),
    )?;

    alice.write_line("/files")?;
    alice.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;

    Ok(())
}

fn parse_file_offer_line(line: &str) -> Option<String> {
    let prefix = "收到文件请求: id=";
    let rest = line.strip_prefix(prefix)?;
    let (file_id, _) = rest.split_once(", from=")?;
    Some(file_id.trim().to_string())
}

fn timeout_command() -> String {
    if cfg!(windows) {
        "ping 127.0.0.1 -n 6 >nul".to_string()
    } else {
        "sleep 3".to_string()
    }
}

fn missing_command_name() -> &'static str {
    "definitely_missing_y2m_command_12345"
}

fn missing_command_exit_code() -> i32 {
    if cfg!(windows) { 1 } else { 127 }
}
