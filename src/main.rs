use server_dot::server::GameServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = GameServer::new(None).await?;
    server.run().await?;
    Ok(())
}
