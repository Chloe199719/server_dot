use crate::game_state::Position;

#[derive(Debug, Clone)]
pub struct PlayerPosition {
    pub id: Vec<u8>,
    pub position: Position,
}
impl PlayerPosition {
    pub fn new(id: Vec<u8>, position: Position) -> Self {
        PlayerPosition { id, position }
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(18 + 8);
        buf.extend_from_slice(&self.id);
        buf.extend_from_slice(&self.position.serialize());
        buf
    }
    pub fn deserialize(data: &[u8]) -> Option<PlayerPosition> {
        if data.len() < 26 {
            return None;
        }
        let id = data[..18].to_vec();
        let position = Position::deserialize(&data[18..])?;
        Some(PlayerPosition { id, position })
    }
}
