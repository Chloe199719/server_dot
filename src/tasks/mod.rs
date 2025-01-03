use std::{sync::Arc, time::Duration};

use tokio::{net::UdpSocket, sync::Mutex, time};

use crate::{
    game_state::{GameState, CLEANUP_INTERVAL_SECS},
    packet::{GamePacket, MessageType},
};

pub async fn handle_cleanup_task(
    cleanup_state: Arc<Mutex<GameState>>,
    cleanup_socket: Arc<UdpSocket>,
) {
    let interval = time::interval(Duration::from_secs(CLEANUP_INTERVAL_SECS));
    tokio::pin!(interval);

    loop {
        interval.tick().await;
        let mut state = cleanup_state.lock().await;
        if let Err(e) = state.cleanup_inactive_players(&cleanup_socket).await {
            tracing::error!("Failed to cleanup inactive players: {e}");
        }
    }
}

pub async fn handle_heartbeat_task(ping_state: Arc<Mutex<GameState>>, ping_socket: Arc<UdpSocket>) {
    let interval = time::interval(Duration::from_secs(3));
    tokio::pin!(interval);

    loop {
        interval.tick().await;
        let state = ping_state.lock().await;
        for (addr, player) in &state.players {
            let reply = GamePacket::new(
                MessageType::Heartbeat,
                0,
                vec![],
                player.id.as_bytes().to_vec(),
            );
            let data = reply.serialize();
            if let Ok(addr) = addr.parse::<std::net::SocketAddr>() {
                if let Err(e) = ping_socket.send_to(&data, addr).await {
                    tracing::error!("Failed to send heartbeat: {addr}: {e}");
                }
            }
        }
    }
}
pub struct HeartbeatManager {
    socket: Arc<UdpSocket>,
    game_state: Arc<Mutex<GameState>>,
}

impl HeartbeatManager {
    pub fn new(socket: Arc<UdpSocket>, game_state: Arc<Mutex<GameState>>) -> Self {
        Self { socket, game_state }
    }

    pub async fn run(&self) {
        let interval = tokio::time::interval(Duration::from_secs(3));
        tokio::pin!(interval);

        loop {
            interval.tick().await;
            if let Err(e) = self.send_heartbeats().await {
                tracing::error!("Failed to send heartbeats: {e}");
            }
        }
    }

    async fn send_heartbeats(&self) -> Result<(), Box<dyn std::error::Error>> {
        let state = self.game_state.lock().await;
        for (addr, player) in &state.players {
            let reply = GamePacket::new(
                MessageType::Heartbeat,
                0,
                vec![],
                player.id.as_bytes().to_vec(),
            );
            let data = reply.serialize();

            if let Ok(addr) = addr.parse::<std::net::SocketAddr>() {
                self.socket.send_to(&data, addr).await?;
            }
        }
        Ok(())
    }
}
