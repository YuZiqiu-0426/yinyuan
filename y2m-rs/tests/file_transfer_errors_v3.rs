use uuid::Uuid;
use y2m_client_core::{
    build_ack_packet, build_file_accept_event_packet, build_file_complete_event_packet,
    build_file_offer_event_packet,
};
use y2m_common::{AckStatus, BinaryChunkHeader, ErrorCode, EventType, PacketKind};
use y2m_server::ServerConfig;

mod support;

use support::{connect_runtime, spawn_server, spawn_server_with_config};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial]
async fn file_chunk_before_accept_returns_error_and_is_not_forwarded() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server().await?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let mut bob_runtime = connect_runtime(server_url.clone(), "group1", "bob", vec![]).await?;

    let file_id = Uuid::new_v4();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "pending.bin", 4, "application/octet-stream", "pending-sha", 4, 1,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;
    let _ = bob_runtime.recv_next_packet().await.expect("file offer");

    let payload = b"ping".to_vec();
    let chunk = BinaryChunkHeader::new(file_id, 0, 1, payload.len() as u32).encode_with_payload(&payload);
    alice_runtime.connection().send_binary(chunk)?;

    let error_packet = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match alice_runtime.recv_next_packet().await.expect("incoming packet") {
                y2m_client_core::IncomingServerPacket::Error(packet) => break packet,
                _ => {}
            }
        }
    }).await?;
    assert_eq!(error_packet.payload.code, ErrorCode::FileTransferNotAccepted);

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        bob_runtime.recv_binary_frame(),
    ).await;
    assert!(result.is_err(), "chunk should not be forwarded before accept");

    server_task.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial]
async fn invalid_binary_chunk_returns_invalid_message_error() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server().await?;

    let mut alice_runtime = connect_runtime(server_url, "group1", "alice", vec![]).await?;
    alice_runtime.connection().send_binary(vec![1, 2, 3])?;

    let error_packet = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match alice_runtime.recv_next_packet().await.expect("incoming packet") {
                y2m_client_core::IncomingServerPacket::Error(packet) => break packet,
                _ => {}
            }
        }
    }).await?;
    assert_eq!(error_packet.payload.code, ErrorCode::InvalidMessage);

    server_task.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial]
async fn binary_chunk_payload_size_mismatch_returns_error_and_is_not_forwarded() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server().await?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let mut bob_runtime = connect_runtime(server_url.clone(), "group1", "bob", vec![]).await?;

    let file_id = Uuid::new_v4();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "payload-size.bin", 8, "application/octet-stream", "payload-size-sha", 4, 2,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;
    let _ = bob_runtime.recv_next_packet().await.expect("file offer");

    let accept_packet = build_file_accept_event_packet(
        bob_runtime.identity(), Some("group1".to_string()), Some("alice".to_string()),
        file_id, "./downloads/payload-size.bin",
    );
    bob_runtime.connection().send_json_packet(&accept_packet)?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let payload = b"ping".to_vec();
    let bad_chunk = BinaryChunkHeader::new(file_id, 0, 2, payload.len() as u32 + 1).encode_with_payload(&payload);
    alice_runtime.connection().send_binary(bad_chunk)?;

    let error_packet = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match alice_runtime.recv_next_packet().await.expect("incoming packet") {
                y2m_client_core::IncomingServerPacket::Error(packet) => break packet,
                _ => {}
            }
        }
    }).await?;
    assert_eq!(error_packet.payload.code, ErrorCode::InvalidMessage);

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        bob_runtime.recv_binary_frame(),
    ).await;
    assert!(result.is_err(), "chunk with mismatched payload size should not be forwarded");

    let good_chunk = BinaryChunkHeader::new(file_id, 0, 2, payload.len() as u32).encode_with_payload(&payload);
    alice_runtime.connection().send_binary(good_chunk)?;
    let received = bob_runtime.recv_binary_frame().await.expect("valid chunk");
    let (header, received_payload) = BinaryChunkHeader::decode(&received).expect("valid chunk");
    assert_eq!(header.file_id, file_id);
    assert_eq!(header.chunk_index, 0);
    assert_eq!(received_payload, payload.as_slice());

    server_task.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial]
async fn file_complete_without_transfer_state_returns_error() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server().await?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let mut bob_runtime = connect_runtime(server_url, "group1", "bob", vec![]).await?;

    let file_id = Uuid::new_v4();
    let complete_packet = build_file_complete_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, 4, "ghost-sha",
    );
    alice_runtime.connection().send_json_packet(&complete_packet)?;

    let error_packet = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match alice_runtime.recv_next_packet().await.expect("incoming packet") {
                y2m_client_core::IncomingServerPacket::Error(packet) => break packet,
                _ => {}
            }
        }
    }).await?;
    assert_eq!(error_packet.payload.code, ErrorCode::FileTransferNotAccepted);

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        bob_runtime.recv_next_packet(),
    ).await;
    assert!(result.is_err(), "file_complete without transfer state should not be forwarded");

    server_task.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial]
async fn file_offer_exceeding_server_limit_returns_file_too_large() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server_with_config(ServerConfig {
        max_file_size: 3,
        ..ServerConfig::default()
    }).await?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let mut bob_runtime = connect_runtime(server_url, "group1", "bob", vec![]).await?;

    let file_id = Uuid::new_v4();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "too-large.bin", 4, "application/octet-stream", "too-large-sha", 4, 1,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;

    let error_packet = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match alice_runtime.recv_next_packet().await.expect("incoming packet") {
                y2m_client_core::IncomingServerPacket::Error(packet) => break packet,
                _ => {}
            }
        }
    }).await?;
    let expected_file_id = file_id.to_string();
    assert_eq!(error_packet.payload.code, ErrorCode::FileTooLarge);
    assert_eq!(
        error_packet.payload.details.get("fileId").and_then(|v| v.as_str()),
        Some(expected_file_id.as_str())
    );

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        bob_runtime.recv_next_packet(),
    ).await;
    assert!(result.is_err(), "file offer should not be forwarded after size rejection");

    server_task.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial]
async fn rejected_file_complete_ack_cleans_transfer_state() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server().await?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let mut bob_runtime = connect_runtime(server_url, "group1", "bob", vec![]).await?;

    let file_id = Uuid::new_v4();
    let offer_packet = build_file_offer_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, "cleanup.bin", 4, "application/octet-stream", "cleanup-sha", 4, 1,
    );
    alice_runtime.connection().send_json_packet(&offer_packet)?;
    let _ = bob_runtime.recv_next_packet().await.expect("file offer");

    let accept_packet = build_file_accept_event_packet(
        bob_runtime.identity(), Some("group1".to_string()), Some("alice".to_string()),
        file_id, "./downloads/cleanup.bin",
    );
    bob_runtime.connection().send_json_packet(&accept_packet)?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let complete_packet = build_file_complete_event_packet(
        alice_runtime.identity(), Some("group1".to_string()), Some("bob".to_string()),
        file_id, 4, "cleanup-sha",
    );
    let complete_request_id = complete_packet.request_id.clone();
    alice_runtime.connection().send_json_packet(&complete_packet)?;

    loop {
        match bob_runtime.recv_next_packet().await.expect("incoming packet") {
            y2m_client_core::IncomingServerPacket::Event(event) if event.payload.event_type == EventType::FileComplete => break,
            _ => {}
        }
    }

    let ack_packet = build_ack_packet(
        bob_runtime.identity(), Some("group1".to_string()), Some("alice".to_string()),
        complete_request_id.clone(), PacketKind::Event, Some(EventType::FileComplete), AckStatus::Rejected,
    );
    bob_runtime.connection().send_json_packet(&ack_packet)?;

    loop {
        match alice_runtime.recv_next_packet().await.expect("incoming packet") {
            y2m_client_core::IncomingServerPacket::Ack(packet) if packet.request_id == complete_request_id => {
                assert_eq!(packet.payload.status, AckStatus::Rejected);
                break;
            }
            _ => {}
        }
    }

    let payload = b"late".to_vec();
    let chunk = BinaryChunkHeader::new(file_id, 0, 1, payload.len() as u32).encode_with_payload(&payload);
    alice_runtime.connection().send_binary(chunk)?;

    let error_packet = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match alice_runtime.recv_next_packet().await.expect("incoming packet") {
                y2m_client_core::IncomingServerPacket::Error(packet) => break packet,
                _ => {}
            }
        }
    }).await?;
    assert_eq!(error_packet.payload.code, ErrorCode::FileTransferNotAccepted);

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        bob_runtime.recv_binary_frame(),
    ).await;
    assert!(result.is_err(), "transfer should be cleaned after rejected ack");

    server_task.abort();
    Ok(())
}
