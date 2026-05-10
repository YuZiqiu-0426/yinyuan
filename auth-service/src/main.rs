use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    auth_service::run().await
}
