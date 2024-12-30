use crate::game_state::{Player, Position};

use super::{GamePacket, MessageType};
#[derive(Debug)]
pub struct ConnectionInitPacketReceived {
    pub msg_type: MessageType,
    pub version: u8,
    pub seq_num: u32,
}

impl ConnectionInitPacketReceived {
    pub fn deserialize(data: &[u8]) -> Option<ConnectionInitPacketReceived> {
        if data.len() < 6 {
            return None; // Not enough for header
        }
        let msg_type = MessageType::from_byte(data[0])?;
        let version = data[1];
        let seq_num = u32::from_be_bytes([data[5], data[4], data[3], data[2]]);
        Some(ConnectionInitPacketReceived {
            msg_type,
            version,
            seq_num,
        })
    }
}

pub struct ConnectionInitPacketSent {
    pub msg_type: MessageType,
    pub version: u8,
    pub seq_num: u32,
    pub client_id: Vec<u8>,
    pub players: Vec<Player>,
}

impl ConnectionInitPacketSent {
    pub fn serialize(&self) -> GamePacket {
        let mut buf = Vec::with_capacity(18 * self.players.len());
        for player in &self.players {
            buf.extend_from_slice(&player.id.as_bytes());
            buf.extend_from_slice(&player.position.serialize());
        }

        GamePacket::new(self.msg_type, self.seq_num, buf, self.client_id.clone())
    }
    pub fn new(seq_num: u32, client_id: Vec<u8>, players: Vec<Player>) -> Self {
        ConnectionInitPacketSent {
            msg_type: MessageType::ConnectionInit,
            version: 1,
            seq_num,
            client_id,
            players,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionInitSync {
    client_id: Vec<u8>,
    position: Position,
}
impl ConnectionInitSync {
    pub fn new(client_id: Vec<u8>, position: Position) -> Self {
        ConnectionInitSync {
            client_id,
            position,
        }
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(18 + 8);
        buf.extend_from_slice(&self.client_id);
        buf.extend_from_slice(&self.position.serialize());
        buf
    }
    pub fn deserialize(data: &[u8]) -> Option<ConnectionInitSync> {
        if data.len() < 26 {
            return None;
        }
        let client_id = data[..18].to_vec();
        let position = Position::deserialize(&data[18..])?;
        Some(ConnectionInitSync {
            client_id,
            position,
        })
    }
}
