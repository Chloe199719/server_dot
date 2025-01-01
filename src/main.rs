use server_dot::{server::GameServer, telemetry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = telemetry::get_subscriber(false);
    telemetry::init_subscriber(subscriber);
    let server = GameServer::new(None).await?;
    server.run().await?;
    Ok(())
}
