use std::sync::Arc;

use tokio::sync::mpsc;
use y2m_common::EventType;

mod support;

use support::{
    assert_metadata_superset, assert_sender_envelope_keys, connect_runtime, recv_event, spawn_dispatch_loop,
    spawn_server, CaptureEventPlugin,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn text_unicast_end_to_end() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server().await?;

    let (alice_tx, mut alice_rx) = mpsc::unbounded_channel();
    let alice_runtime = connect_runtime(
        server_url.clone(),
        "group1",
        "alice",
        vec![Arc::new(CaptureEventPlugin {
            tx: alice_tx,
            supported: &[EventType::Text],
        })],
    )
    .await?;
    let bob_runtime = connect_runtime(server_url.clone(), "group1", "bob", vec![]).await?;

    let alice_dispatch = spawn_dispatch_loop(alice_runtime);

    bob_runtime.send_text(Some("group1".to_string()), Some("alice".to_string()), "hello from bob")?;

    let received = recv_event(&mut alice_rx).await?;
    assert_eq!(received.group, "group1");
    assert_eq!(received.from, "bob");
    assert_eq!(received.event_type, EventType::Text);
    assert_eq!(received.content, serde_json::json!("hello from bob"));
    assert_metadata_superset(
        &received.metadata,
        serde_json::json!({ "contentType": "text/plain" }),
    );
    assert_sender_envelope_keys(&received.metadata);

    alice_dispatch.abort();
    server_task.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn text_broadcast_end_to_end() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server().await?;

    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel();
    let alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    let bob_runtime = connect_runtime(
        server_url.clone(),
        "group1",
        "bob",
        vec![Arc::new(CaptureEventPlugin {
            tx: bob_tx,
            supported: &[EventType::Text],
        })],
    )
    .await?;

    let bob_dispatch = spawn_dispatch_loop(bob_runtime);

    alice_runtime.send_text(None, None, "hello everyone")?;

    let received = recv_event(&mut bob_rx).await?;
    assert_eq!(received.group, "group1");
    assert_eq!(received.from, "alice");
    assert_eq!(received.event_type, EventType::Text);
    assert_eq!(received.content, serde_json::json!("hello everyone"));
    assert_metadata_superset(
        &received.metadata,
        serde_json::json!({ "contentType": "text/plain" }),
    );
    assert_sender_envelope_keys(&received.metadata);

    bob_dispatch.abort();
    server_task.abort();
    Ok(())
}
