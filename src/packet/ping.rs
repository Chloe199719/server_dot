#[derive(Debug, Clone)]
pub struct PlayerLeft {
    pub player_id: String,
}

impl PlayerLeft {
    #[must_use]
    pub fn new(player_id: String) -> Self {
        PlayerLeft { player_id }
    }
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(18);
        buf.extend_from_slice(self.player_id.as_bytes());
        buf
    }
    #[must_use]
    pub fn deserialize(data: &[u8]) -> Option<PlayerLeft> {
        if data.len() < 18 {
            return None;
        }
        let player_id = String::from_utf8(data[..18].to_vec()).ok()?;
        Some(PlayerLeft { player_id })
    }
}
