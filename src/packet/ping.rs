pub struct PingPayload {
    pub timestamp: u64,
}
#[derive(Debug, Clone)]
pub struct PlayerLeft {
    pub player_id: String,
}

impl PlayerLeft {
    pub fn new(player_id: String) -> Self {
        PlayerLeft { player_id }
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(18);
        buf.extend_from_slice(self.player_id.as_bytes());
        buf
    }
    pub fn deserialize(data: &[u8]) -> Option<PlayerLeft> {
        if data.len() < 18 {
            return None;
        }
        let player_id = String::from_utf8(data[..18].to_vec()).ok()?;
        Some(PlayerLeft { player_id })
    }
}
