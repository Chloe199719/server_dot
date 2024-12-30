use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use server_dot::{
    game_state::{self, Player, Position},
    packet::{
        connection_init::{ConnectionInitPacketSent, ConnectionInitSync},
        ping::PlayerLeft,
        position::PlayerPosition,
        GamePacket, MessageType, PositionGamePacket,
    },
};
use tokio::{net::UdpSocket, sync::Mutex, task, time};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = "0.0.0.0:5000";
    let socket = Arc::new(UdpSocket::bind(server_addr).await?);
    let game_state = Arc::new(Mutex::new(game_state::GameState::default()));
    let cleanup_state = Arc::clone(&game_state);
    let cleanup_socket = Arc::clone(&socket);
    task::spawn(async move {
        let interval = time::interval(Duration::from_secs(5));
        tokio::pin!(interval);

        loop {
            interval.tick().await;
            let mut state = cleanup_state.lock().await;
            let now = Instant::now();
            let ids_to_remove: HashMap<String, Player> = state
                .players
                .iter()
                .filter(|(_, player)| {
                    now.duration_since(player.heartbeat) > Duration::from_secs(10)
                })
                .map(|(addr, player)| (addr.clone(), player.clone()))
                .collect();
            for id in ids_to_remove {
                let player_left_payload = PlayerLeft::new(id.1.id);
                for (addr, p) in &state.players {
                    if addr != &id.0 {
                        let packet = GamePacket::new(
                            MessageType::PlayerLeft,
                            0,
                            player_left_payload.serialize(),
                            p.id.as_bytes().to_vec(),
                        );
                        let data = packet.serialize();
                        cleanup_socket.send_to(&data, addr).await.unwrap();
                    }
                }
            }
            state.players.retain(|_addr, player| {
                if now.duration_since(player.heartbeat) > Duration::from_secs(10) {
                    // println!("Removing inactive player: {}", addr);
                    false
                } else {
                    true
                }
            });
        }
    });
    // Start a Task to ping all players
    // Start a task for sending heartbeats
    let ping_socket = Arc::clone(&socket);
    let ping_state = Arc::clone(&game_state);
    task::spawn(async move {
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
                        eprintln!("Failed to send heartbeat to {}: {}", addr, e);
                    }
                }
            }
        }
    });
    loop {
        let mut buf = vec![0; 1024];
        let (len, addr) = socket.recv_from(&mut buf).await?;
        // println!("Received {:?} bytes , len {}", &buf[..len].to_vec(), len);
        let package = GamePacket::deserialize(&buf[..len]).unwrap();
        match package.msg_type {
            MessageType::PositionUpdate => {
                let package = PositionGamePacket::new(package);
                let game_state = game_state.lock().await;
                let position_payload =
                    PlayerPosition::new(package.client_id.to_vec(), package.position);

                for (player_id, player) in game_state.players.iter() {
                    if player.id != String::from_utf8(package.client_id.clone()).unwrap() {
                        let position_packet = GamePacket::new(
                            MessageType::PositionUpdate,
                            package.seq_num,
                            position_payload.serialize(),
                            player_id.as_bytes().try_into().unwrap(),
                        );

                        match socket
                            .send_to(&position_packet.serialize(), player_id)
                            .await
                        {
                            Ok(_) => {
                                // println!("Position packet sent");
                            }
                            Err(e) => println!("Error sending position packet: {:?}", e),
                        }
                    }
                }
            }
            MessageType::ChatMessage => {
                println!("Chat message: {:?}", package);
            }
            MessageType::Heartbeat => {
                // Update heartbeat
                println!("Heartbeat from {:?}", addr);
                let mut state = game_state.lock().await;
                println!("Player count: {}", state.players.len());
                if let Some(player) = state.get_player_mut(&addr.to_string()) {
                    player.heartbeat = Instant::now();
                }
            }
            MessageType::ConnectionInit => {
                let mut game_state = game_state.lock().await;
                let player = game_state::Player {
                    id: nanoid::nanoid!(18),
                    position: Position { x: 700.0, y: 700.0 },
                    heartbeat: std::time::Instant::now(),
                    seq_num: package.seq_num,
                };
                let player_id = player.id.clone();
                game_state.add_player(player, addr.to_string());

                let players = game_state
                    .get_players()
                    .iter()
                    .filter(|(__, player)| player.id != player_id)
                    .map(|(_, p)| p.clone())
                    .collect::<Vec<Player>>();

                socket
                    .send_to(
                        &ConnectionInitPacketSent::new(
                            package.seq_num,
                            player_id.as_bytes().to_vec(),
                            players,
                        )
                        .serialize()
                        .serialize(),
                        addr,
                    )
                    .await?;

                for (send_addr, player) in game_state.players.iter() {
                    let connection_payload = ConnectionInitSync::new(
                        player_id.as_bytes().to_vec(),
                        player.position.clone(),
                    );

                    if player_id != player.id {
                        // println!("Position {:?}", connection_payload.serialize());

                        let connection_packet = GamePacket::new(
                            MessageType::PlayerJoin,
                            package.seq_num,
                            connection_payload.serialize(),
                            player.id.as_bytes().to_vec(),
                        );
                        match socket
                            .send_to(&connection_packet.serialize(), send_addr)
                            .await
                        {
                            Ok(_) => {
                                // println!(
                                //     "Player join packet sent to {:?} player_id {}",
                                //     addr, player.id
                                // );
                            }
                            Err(e) => println!("Error sending player join packet: {:?}", e),
                        }
                    }
                }
            }

            _ => {
                println!("Unknown message type: {:?}", package);
            }
        }
    }
    // Ok(())
}
