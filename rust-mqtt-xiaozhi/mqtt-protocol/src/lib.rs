use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use anyhow::Result;
use thiserror::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Error, Debug)]
pub enum MqttError {
    #[error("协议错误: {0}")]
    ProtocolError(String),
    
    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("无效的消息类型: {0}")]
    InvalidMessageType(u8),
    
    #[error("消息长度超过限制: {0}")]
    MessageTooLarge(usize),
    
    #[error("缓冲区不完整")]
    IncompleteMessage,
    
    #[error("连接未建立")]
    NotConnected,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum PacketType {
    Connect = 1,
    Connack = 2,
    Publish = 3,
    Puback = 4,
    Pubrec = 5,
    Pubrel = 6,
    Pubcomp = 7,
    Subscribe = 8,
    Suback = 9,
    Unsubscribe = 10,
    Unsuback = 11,
    Pingreq = 12,
    Pingresp = 13,
    Disconnect = 14,
}

impl TryFrom<u8> for PacketType {
    type Error = MqttError;
    
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(PacketType::Connect),
            2 => Ok(PacketType::Connack),
            3 => Ok(PacketType::Publish),
            4 => Ok(PacketType::Puback),
            5 => Ok(PacketType::Pubrec),
            6 => Ok(PacketType::Pubrel),
            7 => Ok(PacketType::Pubcomp),
            8 => Ok(PacketType::Subscribe),
            9 => Ok(PacketType::Suback),
            10 => Ok(PacketType::Unsubscribe),
            11 => Ok(PacketType::Unsuback),
            12 => Ok(PacketType::Pingreq),
            13 => Ok(PacketType::Pingresp),
            14 => Ok(PacketType::Disconnect),
            _ => Err(MqttError::InvalidMessageType(value)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectData {
    pub protocol_name: String,
    pub protocol_level: u8,
    pub connect_flags: u8,
    pub keep_alive: u16,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PublishData {
    pub topic: String,
    pub packet_id: Option<u16>,
    pub qos: u8,
    pub dup: bool,
    pub retain: bool,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SubscribeData {
    pub packet_id: u16,
    pub topic: String,
    pub qos: u8,
}

pub struct MqttProtocol<T> {
    socket: Arc<Mutex<T>>,
    max_payload_size: usize,
    is_connected: bool,
    keep_alive_interval: u16,
    last_activity: Arc<Mutex<std::time::Instant>>,
}

impl<T> MqttProtocol<T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    pub fn new(socket: T, max_payload_size: usize) -> Self {
        Self {
            socket: Arc::new(Mutex::new(socket)),
            max_payload_size,
            is_connected: false,
            keep_alive_interval: 0,
            last_activity: Arc::new(Mutex::new(std::time::Instant::now())),
        }
    }
    
    pub async fn process_buffer(&mut self, buffer: &mut Vec<u8>) -> Result<Option<MqttMessage>> {
        loop {
            if buffer.len() < 2 {
                return Ok(None);
            }
            
            let first_byte = buffer[0];
            let packet_type = (first_byte >> 4) & 0x0F;
            let packet_type = PacketType::try_from(packet_type)?;
            
            let (remaining_length, bytes_read) = self.decode_remaining_length(buffer)?;
            let total_length = 1 + bytes_read + remaining_length;
            
            if total_length > self.max_payload_size {
                return Err(MqttError::MessageTooLarge(total_length).into());
            }
            
            if buffer.len() < total_length {
                return Ok(None);
            }
            
            let message_data = buffer.drain(..total_length).collect::<Vec<u8>>();
            let message = self.parse_message(packet_type, &message_data)?;
            
            *self.last_activity.lock().await = std::time::Instant::now();
            
            return Ok(Some(message));
        }
    }
    
    pub async fn read_packet(&mut self, buffer: &mut Vec<u8>) -> Result<Option<MqttMessage>> {
        let mut read_buffer = [0u8; 8192];
        
        let mut socket = self.socket.lock().await;
        let n = socket.read(&mut read_buffer).await?;
        drop(socket);
        
        if n == 0 {
            return Ok(None);
        }
        
        buffer.extend_from_slice(&read_buffer[..n]);
        
        self.process_buffer(buffer).await
    }
    
    fn decode_remaining_length(&self, buffer: &[u8]) -> Result<(usize, usize), MqttError> {
        let mut multiplier = 1;
        let mut length = 0usize;
        let mut bytes_read = 1usize;
        let mut index = 1;
        
        loop {
            if index >= buffer.len() {
                return Err(MqttError::IncompleteMessage);
            }
            
            let byte = buffer[index];
            length += ((byte & 0x7F) as usize) * multiplier;
            
            if (byte & 0x80) == 0 {
                break;
            }
            
            multiplier *= 128;
            index += 1;
            bytes_read += 1;
            
            if bytes_read > 4 {
                return Err(MqttError::ProtocolError("剩余长度编码错误".to_string()));
            }
        }
        
        Ok((length, bytes_read))
    }
    
    fn parse_message(&self, packet_type: PacketType, data: &[u8]) -> Result<MqttMessage, MqttError> {
        match packet_type {
            PacketType::Connect => self.parse_connect(data),
            PacketType::Publish => self.parse_publish(data),
            PacketType::Subscribe => self.parse_subscribe(data),
            PacketType::Pingreq => Ok(MqttMessage::Pingreq),
            PacketType::Disconnect => Ok(MqttMessage::Disconnect),
            _ => Ok(MqttMessage::Unknown(packet_type)),
        }
    }
    
    fn parse_connect(&self, data: &[u8]) -> Result<MqttMessage, MqttError> {
        let mut cursor = 0;
        
        // 检查协议名称长度
        if cursor + 1 > data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let protocol_name_len = data[cursor] as usize;
        cursor += 1;
        
        if cursor + protocol_name_len > data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let protocol_name = String::from_utf8_lossy(&data[cursor..cursor + protocol_name_len]);
        cursor += protocol_name_len;
        
        // 检查协议级别
        if cursor >= data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let protocol_level = data[cursor];
        cursor += 1;
        
        // 检查连接标志
        if cursor >= data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let connect_flags = data[cursor];
        cursor += 1;
        
        // 检查保活时间
        if cursor + 2 > data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let keep_alive = u16::from_be_bytes([data[cursor], data[cursor + 1]]);
        cursor += 2;
        
        // 检查客户端 ID 长度
        if cursor >= data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let client_id_len = data[cursor] as usize;
        cursor += 1;
        
        if cursor + client_id_len > data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let client_id = String::from_utf8_lossy(&data[cursor..cursor + client_id_len]);
        cursor += client_id_len;
        
        let mut username = None;
        let mut password = None;
        
        if (connect_flags & 0x80) != 0 {
            if cursor >= data.len() {
                return Err(MqttError::IncompleteMessage);
            }
            
            let username_len = data[cursor] as usize;
            cursor += 1;
            
            if cursor + username_len > data.len() {
                return Err(MqttError::IncompleteMessage);
            }
            
            username = Some(String::from_utf8_lossy(&data[cursor..cursor + username_len]));
            cursor += username_len;
        }
        
        if (connect_flags & 0x40) != 0 {
            if cursor >= data.len() {
                return Err(MqttError::IncompleteMessage);
            }
            
            let password_len = data[cursor] as usize;
            cursor += 1;
            
            if cursor + password_len > data.len() {
                return Err(MqttError::IncompleteMessage);
            }
            
            password = Some(String::from_utf8_lossy(&data[cursor..cursor + password_len]));
        }
        
        Ok(MqttMessage::Connect(ConnectData {
            protocol_name: protocol_name.to_string(),
            protocol_level,
            connect_flags,
            keep_alive,
            client_id: client_id.to_string(),
            username: username.map(|s| s.to_string()),
            password: password.map(|s| s.to_string()),
        }))
    }
    
    fn parse_publish(&self, data: &[u8]) -> Result<MqttMessage, MqttError> {
        let mut cursor = 0;
        
        // 检查主题长度
        if cursor + 2 > data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let topic_len = u16::from_be_bytes([data[cursor], data[cursor + 1]]) as usize;
        cursor += 2;
        
        if cursor + topic_len > data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let topic = String::from_utf8_lossy(&data[cursor..cursor + topic_len]);
        cursor += topic_len;
        
        // 检查标志位
        if cursor >= data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let qos = (data[cursor] >> 1) & 0x03;
        let dup = (data[cursor] & 0x01) != 0;
        let retain = (data[cursor] & 0x02) != 0;
        cursor += 1;
        
        let mut packet_id = None;
        if qos > 0 {
            if cursor + 2 > data.len() {
                return Err(MqttError::IncompleteMessage);
            }
            
            packet_id = Some(u16::from_be_bytes([data[cursor], data[cursor + 1]]));
            cursor += 2;
        }
        
        // 剩余的就是 payload
        let payload = data[cursor..].to_vec();
        
        Ok(MqttMessage::Publish(PublishData {
            topic: topic.to_string(),
            packet_id,
            qos,
            dup,
            retain,
            payload,
        }))
    }
    
    fn parse_subscribe(&self, data: &[u8]) -> Result<MqttMessage, MqttError> {
        if data.len() < 4 {
            return Err(MqttError::IncompleteMessage);
        }
        
        let packet_id = u16::from_be_bytes([data[0], data[1]]);
        let topic_len = u16::from_be_bytes([data[2], data[3]]) as usize;
        
        if 4 + topic_len > data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let topic = String::from_utf8_lossy(&data[4..4 + topic_len]);
        
        if 4 + topic_len >= data.len() {
            return Err(MqttError::IncompleteMessage);
        }
        
        let qos = data[4 + topic_len];
        
        Ok(MqttMessage::Subscribe(SubscribeData {
            packet_id,
            topic: topic.to_string(),
            qos,
        }))
    }
    
    pub fn encode_remaining_length(length: usize) -> Vec<u8> {
        let mut encoded = Vec::new();
        let mut value = length;
        let mut byte;
        
        loop {
            byte = (value & 0x7F) as u8;
            value >>= 7;
            if value > 0 {
                byte |= 0x80;
            }
            encoded.push(byte);
            
            if value == 0 {
                break;
            }
        }
        
        encoded
    }
    
    pub async fn send_connack(&self, return_code: u8) -> Result<(), MqttError> {
        let mut buffer = BytesMut::new();
        buffer.put_u8(0x20); // CONNACK
        buffer.put_u8(0x02); // 剩余长度
        buffer.put_u8(0x00); // 连接确认标志
        buffer.put_u8(return_code); // 返回码
        
        let mut socket = self.socket.lock().await;
        socket.write_all(&buffer.to_vec()).await?;
        Ok(())
    }
    
    pub async fn send_suback(&self, packet_id: u16, qos: u8) -> Result<(), MqttError> {
        let mut buffer = BytesMut::new();
        buffer.put_u8(0x90); // SUBACK
        buffer.put_u8(0x02); // 剩余长度
        buffer.put_u16(packet_id);
        buffer.put_u8(qos);
        
        let mut socket = self.socket.lock().await;
        socket.write_all(&buffer.to_vec()).await?;
        Ok(())
    }
    
    pub async fn send_publish(&self, topic: &str, payload: &[u8], qos: u8) -> Result<(), MqttError> {
        let mut buffer = BytesMut::new();
        buffer.put_u8(0x30 | (qos << 1)); // PUBLISH
        buffer.put_u8(0x00); // 剩余长度（简化）
        
        let topic_bytes = topic.as_bytes();
        buffer.put_u16(topic_bytes.len() as u16);
        buffer.put_slice(topic_bytes);
        buffer.put_slice(payload);
        
        let mut socket = self.socket.lock().await;
        socket.write_all(&buffer.to_vec()).await?;
        Ok(())
    }
    
    pub async fn send_pingresp(&self) -> Result<(), MqttError> {
        let mut buffer = BytesMut::new();
        buffer.put_u8(0xD0); // PINGRESP
        buffer.put_u8(0x00); // 剩余长度
        
        let mut socket = self.socket.lock().await;
        socket.write_all(&buffer.to_vec()).await?;
        Ok(())
    }
    
    pub fn set_connected(&mut self, connected: bool) {
        self.is_connected = connected;
    }
    
    pub fn set_keep_alive_interval(&mut self, interval: u16) {
        self.keep_alive_interval = interval;
    }
    
    pub fn get_last_activity(&self) -> std::time::Instant {
        // 在异步环境中使用 block_in_place
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                *self.last_activity.lock().await
            })
        })
    }
}

#[derive(Debug, Clone)]
pub enum MqttMessage {
    Connect(ConnectData),
    Connack(u8),
    Publish(PublishData),
    Subscribe(SubscribeData),
    Suback(u16, u8),
    Pingreq,
    Pingresp,
    Disconnect,
    Unknown(PacketType),
}
