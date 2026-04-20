use std::sync::Arc;

use tokio::sync::mpsc;
use y2m_client_core::build_command_event_packet;
use y2m_common::EventType;

mod support;

use support::{connect_runtime, recv_event, spawn_dispatch_loop, spawn_server, CaptureEventPlugin, CommandResponderPlugin, ReceivedEvent};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn command_result_end_to_end() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server().await?;

    let (alice_tx, mut alice_rx) = mpsc::unbounded_channel();
    let alice_runtime = connect_runtime(
        server_url.clone(),
        "group1",
        "alice",
        vec![Arc::new(CaptureEventPlugin {
            tx: alice_tx,
            supported: &[EventType::CommandResult],
        })],
    )
    .await?;
    let bob_runtime = connect_runtime(
        server_url.clone(),
        "group1",
        "bob",
        vec![Arc::new(CommandResponderPlugin)],
    )
    .await?;

    let command_packet = build_command_event_packet(
        alice_runtime.identity(),
        Some("group1".to_string()),
        Some("bob".to_string()),
        "whoami",
        Some(30),
    );
    let alice_connection = alice_runtime.connection().clone();

    let alice_dispatch = spawn_dispatch_loop(alice_runtime);
    let bob_dispatch = spawn_dispatch_loop(bob_runtime);

    alice_connection.send_json_packet(&command_packet)?;

    let received = recv_event(&mut alice_rx).await?;
    assert_eq!(
        received,
        ReceivedEvent {
            group: "group1".to_string(),
            from: "bob".to_string(),
            event_type: EventType::CommandResult,
            content: serde_json::Value::Null,
            metadata: serde_json::json!({
                "requestId": command_packet.request_id,
                "exitCode": 0,
                "stdout": "echo: whoami",
                "stderr": "",
                "durationMs": 1
            }),
        }
    );

    alice_dispatch.abort();
    bob_dispatch.abort();
    server_task.abort();
    Ok(())
}
