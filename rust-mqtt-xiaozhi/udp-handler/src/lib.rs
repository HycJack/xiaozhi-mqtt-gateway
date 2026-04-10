use tokio::net::UdpSocket;
use bytes::{Buf, BufMut, BytesMut, Bytes};
use anyhow::{Context, Result};
use thiserror::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error, debug};

mod zero_copy;

#[derive(Error, Debug)]
pub enum UdpError {
    #[error("UDP 错误: {0}")]
    UdpError(String),
    
    #[error("加密错误: {0}")]
    EncryptionError(String),
    
    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct UdpConfig {
    pub server: String,
    pub port: u16,
    pub encryption: String,
    pub key: Vec<u8>,
    pub nonce: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct UdpHeader {
    pub msg_type: u8,
    pub flags: u8,
    pub length: u16,
    pub connection_id: u32,
    pub timestamp: u64,
    pub sequence: u32,
}

pub struct UdpHandler {
    socket: Arc<Mutex<Option<Arc<UdpSocket>>>>,
    config: UdpConfig,
    is_initialized: Arc<Mutex<bool>>,
    sequence: Arc<Mutex<u32>>,
    zero_copy_handler: Arc<Mutex<Option<Arc<zero_copy::UdpZeroCopyHandler>>>>,
}

impl UdpHandler {
    pub fn new(config: UdpConfig) -> Self {
        Self {
            socket: Arc::new(Mutex::new(None)),
            config,
            is_initialized: Arc::new(Mutex::new(false)),
            sequence: Arc::new(Mutex::new(0)),
            zero_copy_handler: Arc::new(Mutex::new(None)),
        }
    }
    
    pub async fn bind(&self) -> Result<()> {
        let addr = format!("{}:{}", self.config.server, self.config.port);
        info!("绑定 UDP: {}", addr);
        
        let socket = UdpSocket::bind(&addr)
            .await
            .context("UDP 绑定失败")?;
        
        let socket_arc = Arc::new(socket);
        
        let zero_copy_handler = Arc::new(zero_copy::UdpZeroCopyHandler::new(
            socket_arc.clone(),
            8192, // 8KB 缓冲区
            1024, // 最大 1024 个缓冲区
        ));
        
        let mut socket_guard = self.socket.lock().await;
        *socket_guard = Some(socket_arc);
        
        let mut zero_copy_guard = self.zero_copy_handler.lock().await;
        *zero_copy_guard = Some(zero_copy_handler);
        
        *self.is_initialized.lock().await = true;
        
        info!("UDP 绑定成功");
        Ok(())
    }
    
    pub async fn send_audio(&self, data: Vec<u8>, timestamp: u64) -> Result<()> {
        let header = UdpHeader {
            msg_type: 1,
            flags: 0,
            length: data.len() as u16,
            connection_id: 0,
            timestamp,
            sequence: {
                let mut seq = self.sequence.lock().await;
                let current = *seq;
                *seq = seq.wrapping_add(1);
                current
            },
        };
        
        let encrypted_data = self.encrypt_audio_data(&data, &header)?;
        let packet = self.build_udp_packet(&header, &encrypted_data)?;
        
        let socket_guard = self.socket.lock().await;
        if let Some(socket) = socket_guard.as_ref() {
            let addr = format!("{}:{}", self.config.server, self.config.port);
            socket.send_to(&packet, &addr)
                .await
                .context("发送 UDP 数据失败")?;
            
            debug!("发送 UDP 音频数据: {} 字节", data.len());
        } else {
            return Err(UdpError::UdpError("UDP 未初始化".to_string()).into());
        }
        
        Ok(())
    }
    
    pub async fn receive_audio(&self) -> Result<(Vec<u8>, UdpHeader)> {
        let socket_guard = self.socket.lock().await;
        if let Some(socket) = socket_guard.as_ref() {
            let mut buffer = vec![0u8; 8192];
            let (len, addr) = socket.recv_from(&mut buffer)
                .await
                .context("接收 UDP 数据失败")?;
            
            buffer.truncate(len);
            debug!("收到 UDP 数据: {} 字节 来自 {}", len, addr);
            
            let header = self.parse_udp_header(&buffer)?;
            let encrypted_data = &buffer[16..16 + header.length as usize];
            let audio_data = self.decrypt_audio_data(encrypted_data, &header)?;
            
            Ok((audio_data, header))
        } else {
            Err(UdpError::UdpError("UDP 未初始化".to_string()).into())
        }
    }
    
    // 零拷贝接收 UDP 数据
    pub async fn receive_audio_zero_copy(&self) -> Result<(Vec<u8>, UdpHeader, std::net::SocketAddr)> {
        let zero_copy_guard = self.zero_copy_handler.lock().await;
        if let Some(zero_copy_handler) = zero_copy_guard.as_ref() {
            let (data, addr) = zero_copy_handler.recv_zero_copy().await?;
            debug!("收到零拷贝 UDP 数据: {} 字节 来自 {}", data.len(), addr);
            
            let header = self.parse_udp_header(&data)?;
            let encrypted_data = &data[16..16 + header.length as usize];
            let audio_data = self.decrypt_audio_data(encrypted_data, &header)?;
            
            Ok((audio_data, header, addr))
        } else {
            Err(UdpError::UdpError("零拷贝处理器未初始化".to_string()).into())
        }
    }
    
    // 零拷贝发送 UDP 数据
    pub async fn send_audio_zero_copy(&self, data: &[u8], timestamp: u64, addr: &std::net::SocketAddr) -> Result<()> {
        let header = UdpHeader {
            msg_type: 1,
            flags: 0,
            length: data.len() as u16,
            connection_id: 0,
            timestamp,
            sequence: {
                let mut seq = self.sequence.lock().await;
                let current = *seq;
                *seq = seq.wrapping_add(1);
                current
            },
        };
        
        let encrypted_data = self.encrypt_audio_data(data, &header)?;
        let packet = self.build_udp_packet(&header, &encrypted_data)?;
        
        let zero_copy_guard = self.zero_copy_handler.lock().await;
        if let Some(zero_copy_handler) = zero_copy_guard.as_ref() {
            zero_copy_handler.send_zero_copy(&packet, addr).await?;
            debug!("零拷贝发送 UDP 音频数据: {} 字节", data.len());
        } else {
            return Err(UdpError::UdpError("零拷贝处理器未初始化".to_string()).into());
        }
        
        Ok(())
    }
    
    fn build_udp_packet(&self, header: &UdpHeader, data: &[u8]) -> Result<Vec<u8>> {
        let mut packet = Vec::with_capacity(16 + data.len());
        
        packet.put_u8(header.msg_type);
        packet.put_u8(header.flags);
        packet.put_u16(header.length);
        packet.put_u32(header.connection_id);
        packet.put_u64(header.timestamp);
        packet.put_u32(header.sequence);
        packet.extend_from_slice(data);
        
        Ok(packet)
    }
    
    fn parse_udp_header(&self, data: &[u8]) -> Result<UdpHeader> {
        if data.len() < 16 {
            return Err(UdpError::UdpError("数据太短，无法解析头部".to_string()).into());
        }
        
        Ok(UdpHeader {
            msg_type: data[0],
            flags: data[1],
            length: u16::from_be_bytes([data[2], data[3]]),
            connection_id: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            timestamp: u64::from_be_bytes([
                data[8], data[9], data[10], data[11],
                data[12], data[13], data[14], data[15]
            ]),
            sequence: u32::from_be_bytes([data[16], data[17], data[18], data[19]]),
        })
    }
    
    fn encrypt_audio_data(&self, data: &[u8], header: &UdpHeader) -> Result<Vec<u8>> {
        match self.config.encryption.as_str() {
            "aes-128-ctr" => self.encrypt_aes_128_ctr(data, header),
            "none" => Ok(data.to_vec()),
            _ => Err(UdpError::EncryptionError("不支持的加密算法".to_string()).into()),
        }
    }
    
    fn decrypt_audio_data(&self, data: &[u8], header: &UdpHeader) -> Result<Vec<u8>> {
        match self.config.encryption.as_str() {
            "aes-128-ctr" => self.decrypt_aes_128_ctr(data, header),
            "none" => Ok(data.to_vec()),
            _ => Err(UdpError::EncryptionError("不支持的加密算法".to_string()).into()),
        }
    }
    
    fn encrypt_aes_128_ctr(&self, data: &[u8], _header: &UdpHeader) -> Result<Vec<u8>> {
        if self.config.key.len() != 16 {
            return Err(UdpError::EncryptionError("密钥长度必须为 16 字节".to_string()).into());
        }
        
        let mut encrypted = Vec::with_capacity(data.len());
        let key = &self.config.key;
        let nonce = &self.config.nonce;
        
        let mut counter = [0u8; 16];
        counter[..nonce.len()].copy_from_slice(nonce);
        
        for (i, byte) in data.iter().enumerate() {
            let block_index = i / 16;
            let byte_index = i % 16;
            
            let mut block_counter = counter.clone();
            let block_counter_bytes = (block_index as u32).to_be_bytes();
            block_counter[12..16].copy_from_slice(&block_counter_bytes);
            
            let keystream_byte = block_counter[byte_index];
            encrypted.push(byte ^ keystream_byte);
        }
        
        Ok(encrypted)
    }
    
    fn decrypt_aes_128_ctr(&self, data: &[u8], _header: &UdpHeader) -> Result<Vec<u8>> {
        if self.config.key.len() != 16 {
            return Err(UdpError::EncryptionError("密钥长度必须为 16 字节".to_string()).into());
        }
        
        let mut decrypted = Vec::with_capacity(data.len());
        let key = &self.config.key;
        let nonce = &self.config.nonce;
        
        let mut counter = [0u8; 16];
        counter[..nonce.len()].copy_from_slice(nonce);
        
        for (i, byte) in data.iter().enumerate() {
            let block_index = i / 16;
            let byte_index = i % 16;
            
            let mut block_counter = counter.clone();
            let block_counter_bytes = (block_index as u32).to_be_bytes();
            block_counter[12..16].copy_from_slice(&block_counter_bytes);
            
            let keystream_byte = block_counter[byte_index];
            decrypted.push(byte ^ keystream_byte);
        }
        
        Ok(decrypted)
    }
    
    pub async fn is_initialized(&self) -> bool {
        *self.is_initialized.lock().await
    }
    
    pub async fn close(&self) -> Result<()> {
        let mut socket_guard = self.socket.lock().await;
        if let Some(socket) = socket_guard.take() {
            drop(socket);
            *self.is_initialized.lock().await = false;
            info!("UDP 连接已关闭");
        }
        Ok(())
    }
}

impl Drop for UdpHandler {
    fn drop(&mut self) {
        let socket = self.socket.clone();
        let is_initialized = self.is_initialized.clone();
        
        tokio::spawn(async move {
            if *is_initialized.lock().await {
                let mut socket_guard = socket.lock().await;
                let _ = socket_guard.take();
                *is_initialized.lock().await = false;
            }
        });
    }
}
