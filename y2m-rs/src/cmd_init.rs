use crate::{
    cli::InitArgs,
    util::{load_or_default_config, resolve_config_path},
};

pub(crate) async fn run_init(args: InitArgs) -> anyhow::Result<()> {
    let config_path = resolve_config_path(args.config);
    let mut config = load_or_default_config(&config_path)?;

    if let Some(server_url) = args.server_url { config.server_url = server_url; }
    if let Some(group) = args.group { config.group_name = Some(group); }
    if let Some(client) = args.client { config.client_name = Some(client); }
    if let Some(token) = args.token { config.token = Some(token); }
    if let Some(interval) = args.heartbeat_interval { config.heartbeat_interval_override_sec = Some(interval); }
    if let Some(dir) = args.download_dir { config.download_dir = Some(dir); }

    config.save_to_path(&config_path)?;
    println!("配置已保存到 {}", config_path.display());
    Ok(())
}
