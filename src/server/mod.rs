use std::{sync::Arc, time::Instant};

use tokio::{net::UdpSocket, sync::Mutex, task};

use crate::{
    game_state::{self, GameState, Player},
    packet::{
        connection_init::{ConnectionInitPacketSent, ConnectionInitSync},
        position::PlayerPosition,
        GamePacket, MessageType,
    },
    tasks::{handle_cleanup_task, HeartbeatManager},
};

pub struct GameServer {
    socket: Arc<UdpSocket>,
    game_state: Arc<Mutex<GameState>>,
}

impl GameServer {
    #[tracing::instrument(name = "GameServer New", skip(addr))]
    pub async fn new(addr: Option<&str>) -> Result<Self, anyhow::Error> {
        match addr {
            Some(addr) => {
                tracing::info!("Binding to address: {}", addr);
                let socket = Arc::new(UdpSocket::bind(addr).await?);
                tracing::info!("Socket bound to address: {}", addr);
                let game_state = Arc::new(Mutex::new(game_state::GameState::default()));
                tracing::info!("Game state initialized");
                Ok(Self { socket, game_state })
            }
            None => Self::default().await,
        }
    }
    async fn default() -> Result<Self, anyhow::Error> {
        let server_addr = "0.0.0.0:5000";
        tracing::info!("Binding to address: {}", server_addr);
        let socket = Arc::new(UdpSocket::bind(server_addr).await?);
        tracing::info!("Socket bound to address: {}", server_addr);

        let game_state = Arc::new(Mutex::new(game_state::GameState::default()));
        tracing::info!("Game state initialized");

        Ok(Self { socket, game_state })
    }
    #[tracing::instrument(name = "GameServer Run", skip(self))]
    pub async fn run(&self) -> Result<(), anyhow::Error> {
        tracing::info!("Starting game server");
        tracing::info!("Server listening on: {:?}", self.socket.local_addr()?);

        tracing::info!("Spawning maintenance tasks");
        self.spawn_maintenance_tasks();
        tracing::info!("Spawning message receiving task");
        self.spawn_handle_receiving_messages_task();
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
        // Ok(())
    }
    #[tracing::instrument(name = "GameServer Spawn Maintenance Tasks", skip(self))]
    fn spawn_maintenance_tasks(&self) {
        // Spawn cleanup task
        let cleanup_state = Arc::clone(&self.game_state);
        let cleanup_socket = Arc::clone(&self.socket);

        tokio::spawn(handle_cleanup_task(cleanup_state, cleanup_socket));
        tracing::info!("Spawned cleanup task");
        // Spawn heartbeat manager
        let heartbeat_manager =
            HeartbeatManager::new(Arc::clone(&self.socket), Arc::clone(&self.game_state));
        task::spawn(async move { heartbeat_manager.run().await });
        tracing::info!("Spawned heartbeat manager");
    }
    #[tracing::instrument(name = "GameServer Spawn Handle Receiving Messages Task", skip(self))]
    fn spawn_handle_receiving_messages_task(&self) {
        let socket_for_task = Arc::clone(&self.socket);
        let state_for_task = Arc::clone(&self.game_state);
        tokio::spawn(async move {
            loop {
                let mut buf = vec![0; 1024];
                let (len, addr) = match socket_for_task.recv_from(&mut buf).await {
                    Ok((len, addr)) => (len, addr),
                    Err(e) => {
                        tracing::error!("Error receiving from socket: {:?}", e);
                        continue;
                    }
                };

                let package = match GamePacket::deserialize(&buf[..len]) {
                    Some(package) => package,
                    None => {
                        tracing::error!("Error deserializing packet");
                        continue;
                    }
                };
                match package.msg_type {
                    MessageType::PositionUpdate => {
                        Self::handle_position_update(
                            &package,
                            &socket_for_task,
                            &state_for_task,
                            addr,
                        )
                        .await;
                    }
                    MessageType::Heartbeat => {
                        Self::handle_heartbeat(&state_for_task, addr).await;
                    }
                    MessageType::ConnectionInit => {
                        Self::handle_connection_init(
                            &package,
                            &socket_for_task,
                            &state_for_task,
                            addr,
                        )
                        .await;
                    }
                    _ => {
                        tracing::warn!("Received unknown message type: {:?}", package.msg_type);
                    }
                }
            }
        });
    }
    #[tracing::instrument(name = "GameServer Handle Heartbeat", skip(state_for_task))]
    async fn handle_heartbeat(state_for_task: &Arc<Mutex<GameState>>, addr: std::net::SocketAddr) {
        let mut state = state_for_task.lock().await;

        if let Some(player) = state.get_player_mut(&addr.to_string()) {
            player.heartbeat = Instant::now();
        } else {
            tracing::warn!("Received heartbeat from unknown player: {:?}", addr);
        }
    }
    #[tracing::instrument(
        name = "GameServer Handle Position Update",
        skip(socket_for_task, state_for_task)
    )]
    async fn handle_position_update(
        package: &GamePacket,
        socket_for_task: &Arc<UdpSocket>,
        state_for_task: &Arc<Mutex<GameState>>,
        addr: std::net::SocketAddr,
    ) {
        let package = crate::packet::PositionGamePacket::new(package);

        let mut game_state = state_for_task.lock().await;
        game_state.update_player_position(addr.to_string().as_str(), package.position.clone());
        let position_payload = PlayerPosition::new(package.client_id.to_vec(), package.position);

        for (player_id, player) in game_state.players.iter() {
            if player.id != String::from_utf8(package.client_id.clone()).unwrap() {
                let position_packet = GamePacket::new(
                    MessageType::PositionUpdate,
                    package.seq_num,
                    position_payload.serialize(),
                    player.id.as_bytes().to_vec(),
                );

                match socket_for_task
                    .send_to(&position_packet.serialize(), player_id)
                    .await
                {
                    Ok(_) => {
                        // println!("Position packet sent");
                    }
                    Err(e) => tracing::error!("Error sending position packet: {:?}", e),
                }
            }
        }
    }
    #[tracing::instrument(
        name = "GameServer Handle Connection Init",
        skip(socket_for_task, state_for_task)
    )]
    async fn handle_connection_init(
        package: &GamePacket,
        socket_for_task: &Arc<UdpSocket>,
        state_for_task: &Arc<Mutex<GameState>>,
        addr: std::net::SocketAddr,
    ) {
        let mut game_state = state_for_task.lock().await;
        let player = game_state::Player {
            id: nanoid::nanoid!(18),
            position: game_state::Position { x: 600.0, y: 700.0 },
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
        match socket_for_task
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
            .await
        {
            Ok(_) => {
                // println!("Position packet sent");
            }
            Err(e) => tracing::error!("Error sending position packet: {:?}", e),
        }
        for (send_addr, player) in game_state.players.iter() {
            let connection_payload =
                ConnectionInitSync::new(player_id.as_bytes().to_vec(), player.position.clone());

            if player_id != player.id {
                // println!("Position {:?}", connection_payload.serialize());

                let connection_packet = GamePacket::new(
                    MessageType::PlayerJoin,
                    package.seq_num,
                    connection_payload.serialize(),
                    player.id.as_bytes().to_vec(),
                );
                match socket_for_task
                    .send_to(&connection_packet.serialize(), send_addr)
                    .await
                {
                    Ok(_) => {
                        // println!(
                        //     "Player join packet sent to {:?} player_id {}",
                        //     addr, player.id
                        // );
                    }
                    Err(e) => tracing::error!(
                        "Error sending player join packet: {:?} send_addr {:?}",
                        e,
                        send_addr
                    ),
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        time::{Duration, Instant},
    };

    use game_state::{Player, Position};
    use rand::Rng;

    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let mut rng = rand::thread_rng();
        // Generate a port in the ephemeral range 49152..65535 (inclusive of 65535)
        let random_port = rng.gen_range(49152..=65535);
        let addr = format!("0.0.0.0:{}", random_port);
        let server = GameServer::new(Some(&addr)).await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_socket_binding() {
        let mut rng = rand::thread_rng();
        // Generate a port in the ephemeral range 49152..65535 (inclusive of 65535)
        let random_port = rng.gen_range(49152..=65535);
        let addr = format!("0.0.0.0:{}", random_port);
        let server = GameServer::new(Some(&addr)).await.unwrap();
        assert!(server.socket.local_addr().is_ok());
    }

    #[tokio::test]
    async fn test_initial_game_state() {
        let mut rng = rand::thread_rng();
        // Generate a port in the ephemeral range 49152..65535 (inclusive of 65535)
        let random_port = rng.gen_range(49152..=65535);
        let addr = format!("0.0.0.0:{}", random_port);
        let server = GameServer::new(Some(&addr)).await.unwrap();
        let state = server.game_state.lock().await;
        assert_eq!(state.players.len(), 0);
    }

    #[tokio::test]
    async fn test_spawn_maintenance_tasks() {
        let mut rng = rand::thread_rng();
        // Generate a port in the ephemeral range 49152..65535 (inclusive of 65535)
        let random_port = rng.gen_range(49152..=65535);
        let addr = format!("0.0.0.0:{}", random_port);
        let server = GameServer::new(Some(&addr)).await.unwrap();
        server.spawn_maintenance_tasks();
        // Verify tasks are spawned by checking they don't panic
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    #[tokio::test]
    async fn test_spawn_handle_receiving_messages_task() {
        let mut rng = rand::thread_rng();
        // Generate a port in the ephemeral range 49152..65535 (inclusive of 65535)
        let random_port = rng.gen_range(49152..=65535);
        let addr = format!("0.0.0.0:{}", random_port);
        let server = GameServer::new(Some(&addr)).await.unwrap();
        server.spawn_handle_receiving_messages_task();
        // Verify tasks are spawned by checking they don't panic
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    #[tokio::test]
    async fn test_handle_position_update() {
        // Generate a Valid random port

        let mut rng = rand::thread_rng();
        // Generate a port in the ephemeral range 49152..65535 (inclusive of 65535)
        let random_port = rng.gen_range(49152..=65535);
        let addr = format!("0.0.0.0:{}", random_port);
        let server2 = GameServer::new(Some(&addr)).await.unwrap();
        // Add a player to the game state
        let mut game_state = server2.game_state.lock().await;
        let player = game_state::Player {
            id: nanoid::nanoid!(18),
            position: Position { x: 700.0, y: 700.0 },
            heartbeat: std::time::Instant::now(),
            seq_num: 0,
        };
        game_state.add_player(player, addr.to_string());
        drop(game_state);
        let game_state = Arc::clone(&server2.game_state);
        let socket = Arc::clone(&server2.socket);
        let package = GamePacket {
            msg_type: MessageType::PositionUpdate,
            version: 1,
            client_id: vec![
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
            ],
            seq_num: 0,
            payload: vec![0, 0, 0, 0, 0, 0, 0, 0],
        };
        GameServer::handle_position_update(
            &package,
            &socket,
            &game_state,
            server2.socket.local_addr().unwrap(),
        )
        .await;
        // Verify tasks are spawned by checking they don't panic
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    #[tokio::test]
    async fn test_position_update_broadcast() {
        // Server setup
        let server = Arc::new(GameServer::new(Some("127.0.0.1:5002")).await.unwrap());
        let server_addr = server.socket.local_addr().unwrap();

        // Spawn server task
        let server_handle = {
            let server = server.clone();
            tokio::spawn(async move { server.run().await.unwrap() })
        };

        // Allow server startup
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create test clients
        let clients: Vec<Arc<UdpSocket>> = vec![
            Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap()),
            Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap()),
            Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap()),
        ];

        // Register players
        {
            let mut state = server.game_state.lock().await;
            for (_, client) in clients.iter().enumerate() {
                let player = Player {
                    id: nanoid::nanoid!(18),
                    position: Position { x: 0.0, y: 0.0 },
                    heartbeat: Instant::now(),
                    seq_num: 0,
                };
                state.add_player(player, client.local_addr().unwrap().to_string());
            }
        }

        // Send position update
        let new_pos = Position { x: 100.0, y: 200.0 };
        let player_id = nanoid::nanoid!(18).as_bytes().to_vec();

        let update = GamePacket::new(
            MessageType::PositionUpdate,
            1,
            new_pos.serialize(),
            player_id.clone(),
        );

        clients[0]
            .send_to(&update.serialize(), server_addr)
            .await
            .unwrap();

        // Verify broadcasts with timeout
        for client in &clients[1..] {
            let mut buf = vec![0; 1024];
            match tokio::time::timeout(Duration::from_secs(5), client.recv_from(&mut buf)).await {
                Ok(Ok((len, _))) => {
                    let packet = GamePacket::deserialize(&buf[..len]).unwrap();

                    assert_eq!(packet.msg_type, MessageType::PositionUpdate);

                    let position_packet = PlayerPosition::deserialize(&packet.payload).unwrap();
                    // Assert f32 equality with small epsilon for floating-point comparison
                    let epsilon = 0.0001;

                    assert!((position_packet.position.x - 100.0).abs() < epsilon);
                    assert!((position_packet.position.y - 200.0).abs() < epsilon);
                }
                _ => panic!("Failed to receive broadcast"),
            }
        }

        server_handle.abort();
    }
    #[tokio::test]
    async fn test_handle_heartbeat() {
        // Generate a Valid random port

        let mut rng = rand::thread_rng();
        // Generate a port in the ephemeral range 49152..65535 (inclusive of 65535)
        let random_port = rng.gen_range(49152..=65535);
        let addr = format!("0.0.0.0:{}", random_port);
        let server2 = GameServer::new(Some(&addr)).await.unwrap();
        // Add a player to the game state
        let mut game_state = server2.game_state.lock().await;
        let player = game_state::Player {
            id: nanoid::nanoid!(18),
            position: Position { x: 700.0, y: 700.0 },
            heartbeat: std::time::Instant::now(),
            seq_num: 0,
        };
        game_state.add_player(player, addr.to_string());
        drop(game_state);
        let game_state = Arc::clone(&server2.game_state);
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], random_port));
        GameServer::handle_heartbeat(&game_state, addr).await;
        // Verify tasks are spawned by checking they don't panic
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    #[tokio::test]
    async fn test_handle_connection_init() {
        // Server setup
        let server = Arc::new(GameServer::new(Some("127.0.0.1:5003")).await.unwrap());
        let server_addr = server.socket.local_addr().unwrap();

        // Spawn server task
        let server_handle = {
            let server = server.clone();
            tokio::spawn(async move { server.run().await.unwrap() })
        };

        // Allow server startup
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create test client
        let client = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());

        // Create and send connection init packet
        let init_packet = GamePacket::new(
            MessageType::ConnectionInit,
            1,
            vec![], // Empty payload for connection init
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // Empty client ID for new connection
        );
        client
            .send_to(&init_packet.serialize(), server_addr)
            .await
            .unwrap();

        // Receive response
        let mut buf = vec![0; 1024];
        match tokio::time::timeout(Duration::from_secs(5), client.recv_from(&mut buf)).await {
            Ok(Ok((len, _))) => {
                let packet = GamePacket::deserialize(&buf[..len]).unwrap();
                assert_eq!(packet.msg_type, MessageType::ConnectionInit);
                // Verify player was added to game state
                let state = server.game_state.lock().await;
                assert_eq!(state.players.len(), 1);

                // Get the first player
                let (_, player) = state.players.iter().next().unwrap();

                // Verify player position
                assert_eq!(player.position.x, 600.0);
                assert_eq!(player.position.y, 700.0);

                // Verify sequence number matches
                assert_eq!(player.seq_num, init_packet.seq_num);

                // Verify player ID length (nanoid generates 18 character IDs)
                assert_eq!(player.id.len(), 18);
            }
            _ => panic!("Failed to receive connection init response"),
        }

        server_handle.abort();
    }
    #[tokio::test]
    async fn test_multiple_connection_init_responses() {
        // Server setup
        let server = Arc::new(GameServer::new(Some("127.0.0.1:5004")).await.unwrap());
        let server_addr = server.socket.local_addr().unwrap();

        // Spawn server task
        let server_handle = {
            let server = server.clone();
            tokio::spawn(async move { server.run().await.unwrap() })
        };

        // Allow server startup
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create test clients
        let client1 = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let client2 = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());

        // Empty client ID for new connections
        let empty_client_id = vec![0; 18];

        // Send connection init packets from both clients
        for client in [&client1, &client2] {
            let init_packet = GamePacket::new(
                MessageType::ConnectionInit,
                1,
                vec![],
                empty_client_id.clone(),
            );
            client
                .send_to(&init_packet.serialize(), server_addr)
                .await
                .unwrap();
        }

        // Receive and verify responses
        let mut received_ids = HashSet::new();
        for client in [&client1, &client2] {
            let mut buf = vec![0; 1024];
            match tokio::time::timeout(Duration::from_secs(5), client.recv_from(&mut buf)).await {
                Ok(Ok((len, _))) => {
                    let packet = GamePacket::deserialize(&buf[..len]).unwrap();

                    // Verify packet type
                    assert_eq!(packet.msg_type, MessageType::ConnectionInit);

                    // Verify unique client IDs
                    assert!(!received_ids.contains(&packet.client_id));
                    received_ids.insert(packet.client_id.clone());

                    // Verify non-empty client ID
                    assert_ne!(packet.client_id, empty_client_id);
                }
                _ => panic!("Failed to receive response or timeout"),
            }
        }

        // Verify server state
        let state = server.game_state.lock().await;
        assert_eq!(state.players.len(), 2);

        // Cleanup
        server_handle.abort();
    }
}
