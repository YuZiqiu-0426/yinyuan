use std::sync::Arc;

use tokio::sync::mpsc;
use y2m_client_core::build_json_event_packet;
use y2m_common::EventType;

mod support;

use support::{connect_runtime, json_message, recv_event, spawn_dispatch_loop, spawn_server, CaptureEventPlugin, ReceivedEvent};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn json_unicast_end_to_end() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server().await?;

    let (alice_tx, mut alice_rx) = mpsc::unbounded_channel();
    let alice_runtime = connect_runtime(
        server_url.clone(),
        "group1",
        "alice",
        vec![Arc::new(CaptureEventPlugin {
            tx: alice_tx,
            supported: &[EventType::Json],
        })],
    )
    .await?;
    let bob_runtime = connect_runtime(server_url.clone(), "group1", "bob", vec![]).await?;

    let alice_dispatch = spawn_dispatch_loop(alice_runtime);

    let packet = build_json_event_packet(
        bob_runtime.identity(),
        Some("group1".to_string()),
        Some("alice".to_string()),
        json_message("hello json"),
    );
    bob_runtime.connection().send_json_packet(&packet)?;

    let received = recv_event(&mut alice_rx).await?;
    assert_eq!(
        received,
        ReceivedEvent {
            group: "group1".to_string(),
            from: "bob".to_string(),
            event_type: EventType::Json,
            content: json_message("hello json"),
            metadata: serde_json::json!({
                "contentType": "application/json"
            }),
        }
    );

    alice_dispatch.abort();
    server_task.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn json_broadcast_end_to_end() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server().await?;

    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel();
    let alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let bob_runtime = connect_runtime(
        server_url.clone(),
        "group1",
        "bob",
        vec![Arc::new(CaptureEventPlugin {
            tx: bob_tx,
            supported: &[EventType::Json],
        })],
    )
    .await?;

    let bob_dispatch = spawn_dispatch_loop(bob_runtime);

    let packet = build_json_event_packet(
        alice_runtime.identity(),
        None,
        None,
        json_message("broadcast json"),
    );
    alice_runtime.connection().send_json_packet(&packet)?;

    let received = recv_event(&mut bob_rx).await?;
    assert_eq!(
        received,
        ReceivedEvent {
            group: "group1".to_string(),
            from: "alice".to_string(),
            event_type: EventType::Json,
            content: json_message("broadcast json"),
            metadata: serde_json::json!({
                "contentType": "application/json"
            }),
        }
    );

    bob_dispatch.abort();
    server_task.abort();
    Ok(())
}
