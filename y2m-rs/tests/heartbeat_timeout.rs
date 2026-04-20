mod support;

use std::time::Duration;

use support::{connect_runtime, spawn_server_with_config};
use y2m_common::ErrorCode;
use y2m_server::ServerConfig;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn timed_out_client_is_removed_and_same_name_can_reconnect() -> anyhow::Result<()> {
    let (server_task, server_url) = spawn_server_with_config(ServerConfig {
        heartbeat_interval_sec: 1,
        heartbeat_timeout_sec: 1,
        ..Default::default()
    })
    .await?;

    let mut alice_runtime = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;

    let timeout_error = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match alice_runtime.recv_next_packet().await {
                Some(y2m_client_core::IncomingServerPacket::Error(packet)) => {
                    if packet.payload.code == ErrorCode::HeartbeatTimeout {
                        return Ok::<_, anyhow::Error>(packet);
                    }
                }
                Some(_) => continue,
                None => {
                    return Err(anyhow::anyhow!(
                        "connection closed before heartbeat timeout error"
                    ))
                }
            }
        }
    })
    .await??;
    assert_eq!(timeout_error.payload.code, ErrorCode::HeartbeatTimeout);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let replacement = connect_runtime(server_url.clone(), "group1", "alice", vec![]).await?;
    assert_eq!(replacement.identity().client_name, "alice");

    server_task.abort();
    Ok(())
}
