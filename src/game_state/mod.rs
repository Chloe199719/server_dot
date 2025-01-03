use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::net::UdpSocket;
pub const CLEANUP_INTERVAL_SECS: u64 = 5;
const PLAYER_TIMEOUT_SECS: u64 = 10;
use crate::packet::{ping::PlayerLeft, GamePacket, MessageType};
#[derive(Debug, Clone)]

pub struct GameState {
    pub players: HashMap<String, Player>,
    pub width: u32,
    pub height: u32,
}
impl Default for GameState {
    #[must_use]
    fn default() -> Self {
        GameState::new(1920, 1080)
    }
}
/// Represents the current state of the game, managing players and game dimensions.
///
/// # Fields
///
/// * `players` - A `HashMap` containing all active players, keyed by their network address
/// * `width` - The width of the game world
/// * `height` - The height of the game world
///
/// # Methods
///
/// The `GameState` struct provides methods to:
/// * Create a new game state with specified dimensions
/// * Add and remove players
/// * Update and retrieve player positions
/// * Get information about players and game dimensions
/// * Clean up inactive players automatically
///
/// # Examples
///
/// ```
/// let mut game = GameState::new(800, 600);
/// let player = Player::new("player1");
/// game.add_player(player, "127.0.0.1:8080".to_string());
/// ```
impl GameState {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        GameState {
            players: HashMap::new(),
            width,
            height,
        }
    }

    pub fn add_player(&mut self, player: Player, address: String) {
        self.players.insert(address, player);
    }
    pub fn remove_player(&mut self, player_id: &str) {
        self.players.remove(player_id);
    }
    pub fn update_player_position(&mut self, player_id: &str, new_position: Position) {
        if let Some(player) = self.get_player_mut(player_id) {
            player.position = new_position;
        }
    }
    #[must_use]
    pub fn get_player(&self, player_id: &str) -> Option<&Player> {
        self.players.get(player_id)
    }
    #[must_use]
    pub fn get_player_mut(&mut self, player_id: &str) -> Option<&mut Player> {
        self.players.get_mut(player_id)
    }
    #[must_use]
    pub fn get_player_position(&self, player_id: &str) -> Option<&Position> {
        self.get_player(player_id).map(|p| &p.position)
    }
    #[must_use]
    pub fn get_player_position_mut(&mut self, player_id: &str) -> Option<&mut Position> {
        self.get_player_mut(player_id).map(|p| &mut p.position)
    }
    #[must_use]
    pub fn get_player_count(&self) -> usize {
        self.players.len()
    }
    #[must_use]
    pub fn get_players(&self) -> &HashMap<String, Player> {
        &self.players
    }
    pub fn get_players_mut(&mut self) -> &mut HashMap<String, Player> {
        &mut self.players
    }
    #[must_use]
    pub fn get_width(&self) -> u32 {
        self.width
    }
    #[must_use]
    pub fn get_height(&self) -> u32 {
        self.height
    }
    /// Cleans up inactive players by removing them from the game state.
    /// This method should be called periodically to ensure that players who have disconnected are removed.
    /// The cleanup interval is defined by the `CLEANUP_INTERVAL_SECS` constant.
    /// # Arguments
    /// * `socket` - A reference to the UDP socket used to send messages to clients
    /// # Errors
    /// This method returns an error if there is a problem sending a message to a client.
    pub async fn cleanup_inactive_players(
        &mut self,
        socket: &Arc<UdpSocket>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();

        // Find inactive players
        let inactive_players: Vec<(String, Player)> = self
            .players
            .iter()
            .filter(|(_, player)| {
                now.duration_since(player.heartbeat) > Duration::from_secs(PLAYER_TIMEOUT_SECS)
            })
            .map(|(addr, player)| (addr.clone(), player.clone()))
            .collect();

        // Notify others about players leaving
        for (addr, player) in inactive_players {
            let player_left_payload = PlayerLeft::new(player.id);

            for (target_addr, p) in &self.players {
                if target_addr != &addr {
                    let packet = GamePacket::new(
                        MessageType::PlayerLeft,
                        0,
                        player_left_payload.serialize(),
                        p.id.as_bytes().to_vec(),
                    );
                    socket.send_to(&packet.serialize(), target_addr).await?;
                }
            }
        }

        // Remove inactive players
        self.players.retain(|_, player| {
            now.duration_since(player.heartbeat) <= Duration::from_secs(PLAYER_TIMEOUT_SECS)
        });

        Ok(())
    }
}
#[derive(Debug, Clone)]

pub struct Player {
    pub id: String,
    pub seq_num: u32,
    pub position: Position,
    pub heartbeat: Instant,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}
impl Position {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Position { x, y }
    }
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8);
        buf.extend_from_slice(&self.x.to_be_bytes());
        buf.extend_from_slice(&self.y.to_be_bytes());
        buf
    }
    #[must_use]
    pub fn deserialize(data: &[u8]) -> Option<Position> {
        if data.len() < 8 {
            return None;
        }
        let x = f32::from_be_bytes([data[3], data[2], data[1], data[0]]);
        let y = f32::from_be_bytes([data[7], data[6], data[5], data[4]]);
        Some(Position { x, y })
    }
}
