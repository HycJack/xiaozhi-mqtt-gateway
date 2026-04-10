use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use thiserror::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error, debug};
use std::time::Duration;
use flate2::Compression;
use bytes::Bytes;

mod connection_pool;
mod serialization;
mod compression;

#[derive(Error, Debug)]
pub enum WebSocketError {
    #[error("连接错误: {0}")]
    ConnectionError(String),
    
    #[error("消息发送错误: {0}")]
    SendError(String),
    
    #[error("消息接收错误: {0}")]
    ReceiveError(String),
    
    #[error("JSON 解析错误: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub url: String,
    pub device_id: String,
    pub protocol_version: String,
    pub authorization: String,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    pub data: Vec<u8>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    #[serde(rename = "hello")]
    Hello {
        version: u8,
        audio_params: AudioParams,
        features: Vec<String>,
    },
    
    #[serde(rename = "audio")]
    Audio(AudioData),
    
    #[serde(rename = "text")]
    Text {
        text: String,
        timestamp: u64,
    },
    
    #[serde(rename = "mcp")]
    Mcp {
        payload: serde_json::Value,
    },
    
    #[serde(rename = "goodbye")]
    Goodbye,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioParams {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
}

pub struct WebSocketBridge {
    ws_stream: Arc<Mutex<Option<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    config: WebSocketConfig,
    is_connected: Arc<Mutex<bool>>,
    connection_pool: Option<Arc<connection_pool::WebSocketPool>>,
    connection_id: Arc<Mutex<Option<String>>>,
    compression_handler: Option<Arc<Mutex<compression::CompressionHandler>>>,
    use_compression: bool,
}

impl WebSocketBridge {
    pub fn new(config: WebSocketConfig) -> Self {
        Self {
            ws_stream: Arc::new(Mutex::new(None)),
            config,
            is_connected: Arc::new(Mutex::new(false)),
            connection_pool: None,
            connection_id: Arc::new(Mutex::new(None)),
            compression_handler: None,
            use_compression: false,
        }
    }
    
    // 启用压缩
    pub fn with_compression(mut self, level: Compression, min_compress_size: usize) -> Self {
        self.compression_handler = Some(Arc::new(Mutex::new(
            compression::CompressionHandler::new(level, min_compress_size)
        )));
        self.use_compression = true;
        self
    }
    
    // 初始化连接池
    pub fn with_connection_pool(mut self, max_connections: usize, idle_timeout: Duration) -> Self {
        self.connection_pool = Some(Arc::new(connection_pool::WebSocketPool::new(
            max_connections,
            idle_timeout,
        )));
        self
    }
    
    pub async fn connect(&self) -> Result<()> {
        let url = &self.config.url;
        info!("连接 WebSocket: {}", url);
        
        let (ws_stream, _) = connect_async(url)
            .await
            .context("WebSocket 连接失败")?;
        
        // 如果有连接池，添加到池中
        if let Some(pool) = &self.connection_pool {
            let connection_id = pool.add_connection(ws_stream).await?;
            let mut conn_id = self.connection_id.lock().await;
            *conn_id = Some(connection_id.clone());
            info!("WebSocket 连接已添加到连接池: {}", connection_id);
            // 连接池已经接管了 WebSocketStream，所以这里不再保存
            *self.is_connected.lock().await = true;
            return Ok(());
        }
        
        let mut stream = self.ws_stream.lock().await;
        *stream = Some(ws_stream);
        *self.is_connected.lock().await = true;
        
        info!("WebSocket 连接成功");
        Ok(())
    }
    
    pub async fn send_hello(&self, audio_params: AudioParams) -> Result<()> {
        let message = WebSocketMessage::Hello {
            version: 3,
            audio_params,
            features: self.config.features.clone(),
        };
        
        self.send_message(message).await
    }
    
    pub async fn send_audio(&self, data: Vec<u8>, timestamp: u64) -> Result<()> {
        let message = WebSocketMessage::Audio(AudioData {
            data,
            timestamp,
        });
        
        self.send_message(message).await
    }
    
    pub async fn send_text(&self, text: String, timestamp: u64) -> Result<()> {
        let message = WebSocketMessage::Text {
            text,
            timestamp,
        };
        
        self.send_message(message).await
    }
    
    pub async fn send_mcp(&self, payload: serde_json::Value) -> Result<()> {
        let message = WebSocketMessage::Mcp {
            payload,
        };
        
        self.send_message(message).await
    }
    
    pub async fn send_goodbye(&self) -> Result<()> {
        let message = WebSocketMessage::Goodbye;
        self.send_message(message).await
    }
    
    async fn send_message(&self, message: WebSocketMessage) -> Result<()> {
        // 使用优化的序列化
        let serialized = serialization::serialize_message(&message)?;
        debug!("发送 WebSocket 消息: {} 字节", serialized.len());
        
        let message = if self.use_compression {
            if let Some(compression_handler) = &self.compression_handler {
                let mut handler = compression_handler.lock().await;
                let compressed = handler.compress(&serialized)?;
                Message::Binary(compressed.to_vec())
            } else {
                Message::Text(String::from_utf8(serialized.to_vec())?)
            }
        } else {
            Message::Text(String::from_utf8(serialized.to_vec())?)
        };
        
        let mut stream = self.ws_stream.lock().await;
        if let Some(ws_stream) = stream.as_mut() {
            ws_stream.send(message)
                .await
                .context("发送消息失败")?;
        } else {
            return Err(WebSocketError::ConnectionError("WebSocket 未连接".to_string()).into());
        }
        
        Ok(())
    }
    
    pub async fn send_binary_audio(&self, data: Vec<u8>, timestamp: u64) -> Result<()> {
        let mut buffer = Vec::with_capacity(16 + data.len());
        
        // 构建二进制头部
        buffer.extend_from_slice(&timestamp.to_be_bytes());
        buffer.extend_from_slice(&(data.len() as u32).to_be_bytes());
        buffer.extend_from_slice(&data);
        
        let mut stream = self.ws_stream.lock().await;
        if let Some(ws_stream) = stream.as_mut() {
            ws_stream.send(Message::Binary(buffer))
                .await
                .context("发送二进制数据失败")?;
        } else {
            return Err(WebSocketError::ConnectionError("WebSocket 未连接".to_string()).into());
        }
        
        debug!("发送二进制音频数据: {} 字节", data.len());
        Ok(())
    }
    
    pub async fn receive_message(&self) -> Result<Option<WebSocketMessage>> {
        let mut stream = self.ws_stream.lock().await;
        if let Some(ws_stream) = stream.as_mut() {
            match ws_stream.next().await {
                Some(Ok(message)) => {
                    match message {
                        Message::Text(text) => {
                            // 使用优化的反序列化
                            let ws_message: WebSocketMessage = serialization::deserialize_message(text.as_bytes())?;
                            debug!("收到 WebSocket 文本消息: {} 字节", text.len());
                            Ok(Some(ws_message))
                        }
                        Message::Binary(data) => {
                            debug!("收到 WebSocket 二进制消息: {} 字节", data.len());
                            
                            // 尝试解压缩
                            let data = if self.use_compression {
                                if let Some(compression_handler) = &self.compression_handler {
                                    let mut handler = compression_handler.lock().await;
                                    handler.decompress(&data)?
                                } else {
                                    Bytes::from(data)
                                }
                            } else {
                                Bytes::from(data)
                            };
                            
                            // 尝试解析为 WebSocketMessage
                            match serialization::deserialize_message::<WebSocketMessage>(&data) {
                                Ok(ws_message) => {
                                    Ok(Some(ws_message))
                                }
                                Err(e) => {
                                    debug!("无法解析二进制消息: {}", e);
                                    // 处理其他类型的二进制消息（如音频数据）
                                    Ok(None)
                                }
                            }
                        }
                        Message::Close(_) => {
                            info!("WebSocket 连接关闭");
                            *self.is_connected.lock().await = false;
                            Ok(None)
                        }
                        _ => Ok(None),
                    }
                }
                Some(Err(e)) => {
                    error!("WebSocket 接收消息错误: {}", e);
                    Err(WebSocketError::ReceiveError(e.to_string()).into())
                }
                None => {
                    info!("WebSocket 连接结束");
                    *self.is_connected.lock().await = false;
                    Ok(None)
                }
            }
        } else {
            Err(WebSocketError::ConnectionError("WebSocket 未连接".to_string()).into())
        }
    }
    
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.lock().await
    }
    
    pub async fn disconnect(&self) -> Result<()> {
        let mut stream = self.ws_stream.lock().await;
        if let Some(mut ws_stream) = stream.take() {
            ws_stream.close(None).await?;
            *self.is_connected.lock().await = false;
            info!("WebSocket 连接已关闭");
        }
        Ok(())
    }
}

impl Drop for WebSocketBridge {
    fn drop(&mut self) {
        let is_connected = self.is_connected.clone();
        let ws_stream = self.ws_stream.clone();
        
        tokio::spawn(async move {
            if *is_connected.lock().await {
                let mut stream = ws_stream.lock().await;
                if let Some(mut ws_stream) = stream.take() {
                    let _ = ws_stream.close(None).await;
                }
            }
        });
    }
}
