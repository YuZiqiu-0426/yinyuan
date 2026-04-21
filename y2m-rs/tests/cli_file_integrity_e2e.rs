use std::{fs, path::Path, time::Duration};

mod support;

use y2m_client_core::{
    build_file_complete_event_packet, build_file_offer_event_packet, IncomingServerPacket,
};
use y2m_common::{AckStatus, BinaryChunkHeader, ErrorCode, EventType};

use support::connect_runtime;
use support::cli::{create_temp_dir, init_client_config, spawn_server_process, spawn_y2m, workspace_root, parse_file_offer_line};

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
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
    let mut bob = spawn_y2m(&["chat", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let payload = b"hello checksum failure".to_vec();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "broken.txt", payload.len() as u64, "text/plain", "deadbeef",
        payload.len() as u32, 1,
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

    let chunk = BinaryChunkHeader::new(file_id, 0, 1, payload.len() as u32).encode_with_payload(&payload);
    alice_runtime.connection().send_binary(chunk)?;
    bob.wait_for_contains("接收进度:", Duration::from_secs(10))?;

    let complete_packet = build_file_complete_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, payload.len() as u64,
        "badbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadb",
    );
    let complete_request_id = complete_packet.request_id.clone();
    alice_runtime.connection().send_json_packet(&complete_packet)?;

    bob.wait_for_contains("文件校验失败:", Duration::from_secs(10))?;
    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Ack(packet) if packet.request_id == complete_request_id => {
                assert_eq!(packet.payload.status, AckStatus::Rejected);
                break;
            }
            _ => {}
        }
    }

    assert!(!Path::new(&bob_download_dir.join("broken.txt")).exists());

    let extra_chunk = BinaryChunkHeader::new(file_id, 0, 1, payload.len() as u32).encode_with_payload(&payload);
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

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
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
    let mut bob = spawn_y2m(&["chat", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let payload = b"hello size mismatch".to_vec();
    let declared_size = payload.len() as u64 + 5;
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "size-mismatch.txt", declared_size, "text/plain",
        "d5604a22cf90cf0cc4855ac44474f30129a5b894face69e18ce54d56fedf4448",
        payload.len() as u32, 1,
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

    let chunk = BinaryChunkHeader::new(file_id, 0, 1, payload.len() as u32).encode_with_payload(&payload);
    alice_runtime.connection().send_binary(chunk)?;
    bob.wait_for_contains("接收进度:", Duration::from_secs(10))?;

    let complete_packet = build_file_complete_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, declared_size,
        "d5604a22cf90cf0cc4855ac44474f30129a5b894face69e18ce54d56fedf4448",
    );
    let complete_request_id = complete_packet.request_id.clone();
    alice_runtime.connection().send_json_packet(&complete_packet)?;

    bob.wait_for_contains("文件校验失败:", Duration::from_secs(10))?;
    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Ack(packet) if packet.request_id == complete_request_id => {
                assert_eq!(packet.payload.status, AckStatus::Rejected);
                break;
            }
            _ => {}
        }
    }

    assert!(!Path::new(&bob_download_dir.join("size-mismatch.txt")).exists());

    let extra_chunk = BinaryChunkHeader::new(file_id, 0, 1, payload.len() as u32).encode_with_payload(&payload);
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

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
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
    let mut bob = spawn_y2m(&["chat", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let file_id = uuid::Uuid::new_v4();
    let first_payload = b"chunk-one".to_vec();
    let second_payload = b"chunk-two".to_vec();
    let declared_size = (first_payload.len() + second_payload.len()) as u64;
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "missing-chunk.txt", declared_size, "text/plain", "missing-chunk-sha",
        first_payload.len() as u32, 2,
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
    alice_runtime.connection().send_binary(first_chunk)?;
    bob.wait_for_contains("接收进度:", Duration::from_secs(10))?;

    let complete_packet = build_file_complete_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, declared_size, "missing-chunk-sha",
    );
    let complete_request_id = complete_packet.request_id.clone();
    alice_runtime.connection().send_json_packet(&complete_packet)?;

    bob.wait_for_contains("文件接收失败:", Duration::from_secs(10))?;
    bob.wait_for_contains("reason=分片不完整", Duration::from_secs(10))?;
    bob.write_line("/files")?;
    bob.wait_for_contains("当前没有待处理文件", Duration::from_secs(10))?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            IncomingServerPacket::Ack(packet) if packet.request_id == complete_request_id => {
                assert_eq!(packet.payload.status, AckStatus::Rejected);
                break;
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
