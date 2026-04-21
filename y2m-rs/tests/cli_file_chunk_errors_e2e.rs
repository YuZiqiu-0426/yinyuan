use std::{fs, path::Path, time::Duration};

mod support;

use y2m_client_core::{build_file_offer_event_packet, IncomingServerPacket};
use y2m_common::{BinaryChunkHeader, ErrorCode, EventType};

use support::connect_runtime;
use support::cli::{create_temp_dir, init_client_config, spawn_server_process, spawn_y2m, workspace_root, parse_file_offer_line};

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
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
    let mut bob = spawn_y2m(&["chat", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let first_payload = b"chunk-one".to_vec();
    let second_payload = b"chunk-two".to_vec();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "out-of-order.txt", (first_payload.len() + second_payload.len()) as u64,
        "text/plain", "out-of-order-sha", first_payload.len() as u32, 2,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());
    bob.write_line(&format!("/accept {offered_id}"))?;
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) if event.payload.event_type == EventType::FileAccept => break,
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
            IncomingServerPacket::Event(event) if event.payload.event_type == EventType::FileAbort => {
                assert_eq!(
                    event.payload.metadata.get("reason").and_then(|v| v.as_str()),
                    Some("chunk sequence mismatch: expected 0, got 1")
                );
                break;
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

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
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
    let mut bob = spawn_y2m(&["chat", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let first_payload = b"chunk-one".to_vec();
    let second_payload = b"chunk-two".to_vec();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "duplicate-chunk.txt", (first_payload.len() + second_payload.len()) as u64,
        "text/plain", "duplicate-chunk-sha", first_payload.len() as u32, 2,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());
    bob.write_line(&format!("/accept {offered_id}"))?;
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) if event.payload.event_type == EventType::FileAccept => break,
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
            IncomingServerPacket::Event(event) if event.payload.event_type == EventType::FileAbort => {
                assert_eq!(
                    event.payload.metadata.get("reason").and_then(|v| v.as_str()),
                    Some("chunk sequence mismatch: expected 1, got 0")
                );
                break;
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

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
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
    let mut bob = spawn_y2m(&["chat", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let first_payload = b"chunk-one".to_vec();
    let second_payload = b"chunk-two".to_vec();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "total-chunks-mismatch.txt", (first_payload.len() + second_payload.len()) as u64,
        "text/plain", "total-chunks-mismatch-sha", first_payload.len() as u32, 2,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let offered_id = bob.wait_for_match(Duration::from_secs(10), parse_file_offer_line)?;
    assert_eq!(offered_id, file_id.to_string());
    bob.write_line(&format!("/accept {offered_id}"))?;
    bob.wait_for_contains("已接受文件:", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Event(event) if event.payload.event_type == EventType::FileAccept => break,
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
            IncomingServerPacket::Event(event) if event.payload.event_type == EventType::FileAbort => {
                assert_eq!(
                    event.payload.metadata.get("reason").and_then(|v| v.as_str()),
                    Some("chunk total mismatch: expected 2, got 3")
                );
                break;
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
