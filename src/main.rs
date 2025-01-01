use std::{sync::Arc, time::Instant};

use server_dot::{
    game_state::{self, Player, Position},
    packet::{
        connection_init::{ConnectionInitPacketSent, ConnectionInitSync},
        position::PlayerPosition,
        GamePacket, MessageType, PositionGamePacket,
    },
    tasks::{handle_cleanup_task, HeartbeatManager},
};
use tokio::{net::UdpSocket, sync::Mutex, task};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = "0.0.0.0:5000";
    let socket = Arc::new(UdpSocket::bind(server_addr).await?);
    let game_state = Arc::new(Mutex::new(game_state::GameState::default()));

    // Player cleanup task
    let cleanup_state = Arc::clone(&game_state);
    let cleanup_socket = Arc::clone(&socket);
    task::spawn(handle_cleanup_task(cleanup_state, cleanup_socket));

    // Start a Task to ping all players
    // Start a task for sending heartbeats

    let heartbeat_manager = HeartbeatManager::new(Arc::clone(&socket), Arc::clone(&game_state));
    task::spawn(async move {
        heartbeat_manager.run().await;
    });

    loop {
        let mut buf = vec![0; 1024];
        let (len, addr) = socket.recv_from(&mut buf).await?;
        // println!("Received {:?} bytes , len {}", &buf[..len].to_vec(), len);
        let package = GamePacket::deserialize(&buf[..len]).unwrap();
        match package.msg_type {
            MessageType::PositionUpdate => {
                let package = PositionGamePacket::new(&package);
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
