pub mod connection_init;
pub mod ping;
pub mod position;
use bytes::{BufMut, BytesMut};

use crate::game_state::Position;

// Define an enum for message types.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    PositionUpdate = 0x01,
    ChatMessage = 0x02,
    Heartbeat = 0x03,
    ConnectionInit = 0x04,
    PlayerJoin = 0x05,
    ConfirmPlayerMovement = 0x06,
    PlayerLeft = 0x07,
}

impl MessageType {
    #[must_use]
    pub fn from_byte(b: u8) -> Option<MessageType> {
        match b {
            0x01 => Some(MessageType::PositionUpdate),
            0x02 => Some(MessageType::ChatMessage),
            0x03 => Some(MessageType::Heartbeat),
            0x04 => Some(MessageType::ConnectionInit),
            0x05 => Some(MessageType::PlayerJoin),
            0x06 => Some(MessageType::ConfirmPlayerMovement),
            0x07 => Some(MessageType::PlayerLeft),
            _ => None,
        }
    }
}
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct GamePacket {
    pub msg_type: MessageType,
    pub version: u8,
    pub client_id: Vec<u8>,
    pub seq_num: u32,
    pub payload: Vec<u8>,
}

impl GamePacket {
    #[must_use]
    pub fn new(msg_type: MessageType, seq_num: u32, payload: Vec<u8>, client_id: Vec<u8>) -> Self {
        GamePacket {
            msg_type,
            version: 1,
            seq_num,
            payload,
            client_id,
        }
    }
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        #[allow(clippy::arithmetic_side_effects)]
        let mut buf = BytesMut::with_capacity(1 + 1 + 18 + 4 + self.payload.len());
        #[allow(clippy::as_conversions)]
        buf.put_u8(self.msg_type as u8);
        buf.put_u8(self.version);
        buf.put_slice(&self.client_id);
        buf.put_u32(self.seq_num);
        buf.put_slice(&self.payload);
        buf.to_vec()
    }
    #[must_use]
    pub fn deserialize(data: &[u8]) -> Option<GamePacket> {
        if data.len() < 6 {
            return None; // Not enough for header
        }
        let msg_type = MessageType::from_byte(data[0])?;
        let version = data[1];
        let client_id = &data[2..20];
        let seq_num = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        let payload = data[24..].to_vec();
        Some(GamePacket {
            msg_type,
            seq_num,
            client_id: client_id.into(),
            payload,
            version,
        })
    }
}
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct PositionGamePacket {
    pub msg_type: MessageType,
    pub version: u8,
    pub client_id: Vec<u8>,
    pub seq_num: u32,
    pub position: Position,
}
impl PositionGamePacket {
    #[must_use]
    pub fn new(game_packet: &GamePacket) -> Self {
        let position = Position {
            x: f32::from_be_bytes([
                game_packet.payload[3],
                game_packet.payload[2],
                game_packet.payload[1],
                game_packet.payload[0],
            ]),
            y: f32::from_be_bytes([
                game_packet.payload[7],
                game_packet.payload[6],
                game_packet.payload[5],
                game_packet.payload[4],
            ]),
        };
        PositionGamePacket {
            msg_type: game_packet.msg_type,
            version: game_packet.version,
            client_id: game_packet.client_id.clone(),
            seq_num: game_packet.seq_num,
            position,
        }
    }
}
