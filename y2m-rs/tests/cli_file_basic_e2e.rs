use std::{fs, path::Path, time::Duration};

mod support;

use support::cli::{
    create_temp_dir, init_client_config, spawn_server_process, spawn_y2m, workspace_root,
    parse_file_offer_line,
};

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定；协议覆盖见 tests/file_transfer_v3.rs"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial]
async fn cli_chat_file_accept_end_to_end() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-accept")?;
    let alice_config = temp_dir.join("alice.json");
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;
    init_client_config(&bob_config, &server_url, "group1", "bob", Some(&bob_download_dir))?;

    let alice_config_text = alice_config.to_string_lossy().to_string();
    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(&["chat", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    let mut alice = spawn_y2m(
        &["chat", "--config", alice_config_text.as_str(), "--to", "bob"],
        &workspace_root(),
        &[],
    )?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;
    alice.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let sample_path = temp_dir.join("sample.txt");
    fs::write(&sample_path, b"hello file from cli chat")?;
    let sample_arg = sample_path.to_string_lossy().replace('\\', "/");
    alice.write_line(&format!("/file {}", sample_arg))?;
    alice.wait_for_contains("sample.txt", Duration::from_secs(15))?;

    let file_id = bob.wait_for_match(Duration::from_secs(60), parse_file_offer_line)?;
    bob.write_line("/files")?;
    bob.wait_for_contains(&file_id, Duration::from_secs(10))?;
    bob.write_line(&format!("/accept {file_id}"))?;
    alice.wait_for_contains("file_accept", Duration::from_secs(15))?;
    bob.wait_for_contains("file_complete", Duration::from_secs(30))?;
    alice.wait_for_contains("sample.txt", Duration::from_secs(30))?;

    let saved = fs::read(bob_download_dir.join("sample.txt"))?;
    assert_eq!(saved, b"hello file from cli chat");

    Ok(())
}

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定；协议覆盖见 tests/file_transfer_v3.rs"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial]
async fn cli_send_file_reject_in_chat_end_to_end() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-reject")?;
    let alice_config = temp_dir.join("alice.json");
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;
    init_client_config(&bob_config, &server_url, "group1", "bob", Some(&bob_download_dir))?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(&["chat", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let sample_path = temp_dir.join("reject.txt");
    fs::write(&sample_path, b"reject me")?;
    let alice_config_text = alice_config.to_string_lossy().to_string();
    let sample_text = sample_path.to_string_lossy().to_string();
    let mut alice = spawn_y2m(
        &["send", "--config", alice_config_text.as_str(), "file", "--to", "bob", sample_text.as_str()],
        &workspace_root(),
        &[],
    )?;

    let file_id = bob.wait_for_match(Duration::from_secs(60), parse_file_offer_line)?;
    bob.write_line(&format!("/reject {file_id}"))?;
    alice.wait_for_contains("file_reject", Duration::from_secs(10))?;
    alice.wait_for_contains("rejected by user", Duration::from_secs(10))?;

    let status = alice.wait()?;
    assert!(!status.success(), "send file should fail when target rejects");
    assert!(!Path::new(&bob_download_dir.join("reject.txt")).exists());

    Ok(())
}

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定；协议覆盖见 tests/file_transfer_v3.rs"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial]
async fn cli_chat_file_abort_end_to_end() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("file-abort")?;
    let alice_config = temp_dir.join("alice.json");
    let bob_config = temp_dir.join("bob.json");
    let bob_download_dir = temp_dir.join("bob-downloads");
    fs::create_dir_all(&bob_download_dir)?;

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;
    init_client_config(&bob_config, &server_url, "group1", "bob", Some(&bob_download_dir))?;

    let alice_config_text = alice_config.to_string_lossy().to_string();
    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut alice = spawn_y2m(
        &["chat", "--config", alice_config_text.as_str(), "--to", "bob"],
        &workspace_root(),
        &[],
    )?;
    let mut bob = spawn_y2m(&["chat", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    alice.wait_for_contains("当前会话:", Duration::from_secs(10))?;
    bob.wait_for_contains("当前会话:", Duration::from_secs(10))?;

    let sample_path = temp_dir.join("abort.bin");
    fs::write(&sample_path, vec![b'x'; 32 * 1024 * 1024])?;
    let sample_arg = sample_path.to_string_lossy().replace('\\', "/");
    alice.write_line(&format!("/file {}", sample_arg))?;
    alice.wait_for_contains("abort.bin", Duration::from_secs(15))?;

    let file_id = bob.wait_for_match(Duration::from_secs(60), parse_file_offer_line)?;
    bob.write_line(&format!("/accept {file_id}"))?;
    alice.wait_for_contains("file_accept", Duration::from_secs(15))?;
    bob.wait_for_contains("chunk", Duration::from_secs(30))?;
    bob.write_line(&format!("/abort {file_id}"))?;

    alice.wait_for_contains("file_abort", Duration::from_secs(15))?;
    bob.wait_for_contains("file_abort", Duration::from_secs(15))?;
    assert!(!Path::new(&bob_download_dir.join("abort.bin")).exists());

    Ok(())
}
