use std::time::Duration;

mod support;

use support::cli::{
    create_temp_dir, init_client_config, run_y2m_checked, spawn_server_process, spawn_y2m,
    workspace_root, missing_command_exit_code, missing_command_name, timeout_command,
};

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
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
    init_client_config(&alice_command_config, &server_url, "group1", "alice-command", None)?;
    init_client_config(&bob_config, &server_url, "group1", "bob", None)?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(&["run", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    bob.wait_for_contains("已连接 [group1 / bob]", Duration::from_secs(10))?;

    let alice_text_config_text = alice_text_config.to_string_lossy().to_string();
    let text_output = run_y2m_checked(
        &["send", "--config", alice_text_config_text.as_str(), "text", "--to", "bob", "hello from cli"],
        &workspace_root(),
        &[],
    )?;
    assert!(text_output.contains("已发送到 [group1][bob]"));
    bob.wait_for_contains("[group1][alice-text] hello from cli", Duration::from_secs(10))?;

    let alice_json_config_text = alice_json_config.to_string_lossy().to_string();
    let json_output = run_y2m_checked(
        &["send", "--config", alice_json_config_text.as_str(), "json", "--to", "bob", r#"{"message":"hello json cli"}"#],
        &workspace_root(),
        &[],
    )?;
    assert!(json_output.contains("已发送 JSON 到 [group1][bob]"));
    bob.wait_for_contains(r#"[group1][alice-json] {"message":"hello json cli"}"#, Duration::from_secs(10))?;

    let alice_command_config_text = alice_command_config.to_string_lossy().to_string();
    let command_output = run_y2m_checked(
        &["send", "--config", alice_command_config_text.as_str(), "command", "--to", "bob", "echo hello from cli command"],
        &workspace_root(),
        &[],
    )?;
    assert!(command_output.contains("已发送命令到 [group1][bob]"));
    assert!(command_output.contains("command_result"));
    assert!(command_output.contains(r#""exitCode":0"#));
    assert!(command_output.contains("hello from cli command"));

    Ok(())
}

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_command_timeout_returns_command_result() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("command-timeout")?;
    let alice_config = temp_dir.join("alice.json");
    let bob_config = temp_dir.join("bob.json");

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;
    init_client_config(&bob_config, &server_url, "group1", "bob", None)?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(&["run", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    bob.wait_for_contains("已连接 [group1 / bob]", Duration::from_secs(10))?;

    let alice_config_text = alice_config.to_string_lossy().to_string();
    let command_output = run_y2m_checked(
        &["send", "--config", alice_config_text.as_str(), "command", "--to", "bob", "--timeout", "1", timeout_command().as_str()],
        &workspace_root(),
        &[],
    )?;
    assert!(command_output.contains("已发送命令到 [group1][bob]"));
    assert!(command_output.contains("command_result"));
    assert!(command_output.contains(r#""exitCode":-1"#));
    assert!(command_output.contains("command timed out after 1s"));

    Ok(())
}

#[cfg_attr(
    windows,
    ignore = "Windows：子进程 CLI 输出/控制台编码在自动化环境中不稳定"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_command_failure_returns_command_result() -> anyhow::Result<()> {
    let (_server, server_url) = spawn_server_process()?;
    let temp_dir = create_temp_dir("command-failure")?;
    let alice_config = temp_dir.join("alice.json");
    let bob_config = temp_dir.join("bob.json");

    init_client_config(&alice_config, &server_url, "group1", "alice", None)?;
    init_client_config(&bob_config, &server_url, "group1", "bob", None)?;

    let bob_config_text = bob_config.to_string_lossy().to_string();
    let mut bob = spawn_y2m(&["run", "--config", bob_config_text.as_str()], &workspace_root(), &[])?;
    bob.wait_for_contains("已连接 [group1 / bob]", Duration::from_secs(10))?;

    let alice_config_text = alice_config.to_string_lossy().to_string();
    let missing_command = missing_command_name();
    let command_output = run_y2m_checked(
        &["send", "--config", alice_config_text.as_str(), "command", "--to", "bob", missing_command],
        &workspace_root(),
        &[],
    )?;
    assert!(command_output.contains("已发送命令到 [group1][bob]"));
    assert!(command_output.contains("command_result"));
    assert!(command_output.contains(&format!(r#""exitCode":{}"#, missing_command_exit_code())));
    assert!(command_output.contains(missing_command));

    Ok(())
}
