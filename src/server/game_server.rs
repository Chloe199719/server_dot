use std::time::Instant;
use tokio::net::UdpSocket;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct GameServer {
    socket: Arc<UdpSocket>,
    game_state: Arc<Mutex<GameState>>,
}

impl GameServer {
    pub fn new(socket: UdpSocket) -> Self {
        Self {
            socket: Arc::new(socket),
            game_state: Arc::new(Mutex::new(GameState::new())),
        }
    }

    pub async fn handle_message(&self, package: Package, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        match package.message_type {
            MessageType::ChatMessage => {
                self.handle_chat_message(package).await?;
            }
            MessageType::Heartbeat => {
                self.handle_heartbeat(addr).await?;
            }
            MessageType::ConnectionInit => {
                self.handle_connection_init(package, addr).await?;
            }
        }
        Ok(())
    }

    async fn handle_chat_message(&self, package: Package) -> Result<(), Box<dyn Error>> {
        println!("Chat message: {:?}", package);
        Ok(())
    }

    async fn handle_heartbeat(&self, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        println!("Heartbeat from {:?}", addr);
        let mut state = self.game_state.lock().await;
        println!("Player count: {}", state.players.len());
        
        if let Some(player) = state.get_player_mut(&addr.to_string()) {
            player.heartbeat = Instant::now();
        }
        Ok(())
    }

    async fn handle_connection_init(&self, package: Package, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        let mut game_state = self.game_state.lock().await;
        let player = Player {
            id: nanoid::nanoid!(18),
            position: Position { x: 700.0, y: 700.0 },
            heartbeat: Instant::now(),
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

        self.socket
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
            
        Ok(())
    }
}