use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, Duration};
use uuid::Uuid;
use log::{info, error, debug, warn};
use mqtt_protocol::{MqttProtocol, MqttMessage, MqttError};
use websocket_bridge::{WebSocketBridge, WebSocketConfig, AudioParams};
use udp_handler::{UdpHandler, UdpConfig};
use auth_manager::{AuthManager, AuthCredentials, AuthResult};
use anyhow::{Context, Result};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("连接错误: {0}")]
    ConnectionError(String),
    
    #[error("心跳超时: {0}")]
    HeartbeatTimeout(String),
    
    #[error("认证失败: {0}")]
    AuthenticationFailed(String),
    
    #[error("协议错误: {0}")]
    ProtocolError(#[from] MqttError),
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub connection_id: String,
    pub client_id: String,
    pub mac_address: String,
    pub group_id: String,
    pub uuid: Option<String>,
    pub connected_at: std::time::Instant,
    pub last_activity: std::time::Instant,
    pub keep_alive_interval: u16,
}

pub struct MqttConnection {
    connection_id: String,
    mqtt_protocol: MqttProtocol<TcpStream>,
    auth_manager: Arc<AuthManager>,
    connection_info: Arc<Mutex<Option<ConnectionInfo>>>,
    websocket_bridge: Arc<Mutex<Option<WebSocketBridge>>>,
    udp_handler: Arc<Mutex<Option<UdpHandler>>>,
    is_connected: Arc<Mutex<bool>>,
}

impl MqttConnection {
    pub fn new(
        socket: TcpStream,
        connection_id: String,
        auth_manager: Arc<AuthManager>,
    ) -> Self {
        let mqtt_protocol = MqttProtocol::new(socket, 8192);
        
        Self {
            connection_id,
            mqtt_protocol,
            auth_manager,
            connection_info: Arc::new(Mutex::new(None)),
            websocket_bridge: Arc::new(Mutex::new(None)),
            udp_handler: Arc::new(Mutex::new(None)),
            is_connected: Arc::new(Mutex::new(false)),
        }
    }
    
    pub async fn handle_connection(&mut self) -> Result<()> {
        info!("处理连接: {}", self.connection_id);
        
        let mut buffer = Vec::new();
        
        loop {
            match self.mqtt_protocol.read_packet(&mut buffer).await {
                Ok(Some(message)) => {
                    self.handle_message(message).await?;
                }
                Ok(None) => {
                    info!("客户端断开连接: {}", self.connection_id);
                    break;
                }
                Err(e) => {
                    // 检查是否是 IncompleteMessage 错误，如果是则继续循环
                    if let Some(mqtt_error) = e.downcast_ref::<mqtt_protocol::MqttError>() {
                        if matches!(mqtt_error, mqtt_protocol::MqttError::IncompleteMessage) {
                            debug!("消息缓冲区不完整，继续读取");
                            continue;
                        }
                    }
                    
                    error!("读取消息错误: {:?}", e);
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_message(&mut self, message: MqttMessage) -> Result<()> {
        match message {
            MqttMessage::Connect(connect_data) => {
                self.handle_connect(connect_data).await?;
            }
            MqttMessage::Publish(publish_data) => {
                self.handle_publish(publish_data).await?;
            }
            MqttMessage::Subscribe(subscribe_data) => {
                self.handle_subscribe(subscribe_data).await?;
            }
            MqttMessage::Pingreq => {
                self.handle_pingreq().await?;
            }
            MqttMessage::Disconnect => {
                self.handle_disconnect().await?;
            }
            _ => {
                debug!("忽略消息: {:?}", message);
            }
        }
        Ok(())
    }
    
    async fn handle_connect(&mut self, connect_data: mqtt_protocol::ConnectData) -> Result<()> {
        info!("处理 CONNECT 消息: {}", connect_data.client_id);
        
        let credentials = AuthCredentials {
            client_id: connect_data.client_id.clone(),
            username: connect_data.username.clone(),
            password: connect_data.password.clone(),
        };
        
        let auth_result = self.auth_manager.authenticate(&credentials)
            .context("认证失败")?;
        
        let connection_info = ConnectionInfo {
            connection_id: self.connection_id.clone(),
            client_id: connect_data.client_id.clone(),
            mac_address: auth_result.mac_address.clone(),
            group_id: auth_result.group_id.clone(),
            uuid: auth_result.uuid.clone(),
            connected_at: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
            keep_alive_interval: connect_data.keep_alive,
        };
        
        *self.connection_info.lock().await = Some(connection_info);
        self.mqtt_protocol.set_connected(true);
        self.mqtt_protocol.set_keep_alive_interval(connect_data.keep_alive);
        *self.is_connected.lock().await = true;
        
        self.mqtt_protocol.send_connack(0).await?;
        
        info!("连接建立成功: {} ({})", self.connection_id, connect_data.client_id);
        Ok(())
    }
    
    async fn handle_publish(&mut self, publish_data: mqtt_protocol::PublishData) -> Result<()> {
        debug!("处理 PUBLISH 消息: {} ({} 字节)", publish_data.topic, publish_data.payload.len());
        
        if let Ok(json_str) = std::str::from_utf8(&publish_data.payload) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                self.handle_json_message(&publish_data.topic, &json).await?;
            }
        }
        
        Ok(())
    }
    
    async fn handle_json_message(&mut self, topic: &str, json: &serde_json::Value) -> Result<()> {
        if let Some(msg_type) = json.get("type").and_then(|v| v.as_str()) {
            match msg_type {
                "hello" => {
                    self.handle_hello_message(json).await?;
                }
                "audio" => {
                    self.handle_audio_message(json).await?;
                }
                "text" => {
                    self.handle_text_message(json).await?;
                }
                "mcp" => {
                    self.handle_mcp_message(json).await?;
                }
                "goodbye" => {
                    self.handle_goodbye_message(json).await?;
                }
                _ => {
                    debug!("未知消息类型: {}", msg_type);
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_hello_message(&mut self, json: &serde_json::Value) -> Result<()> {
        info!("处理 Hello 消息");
        
        let session_id = Uuid::new_v4().to_string();
        
        let audio_params = AudioParams {
            codec: "opus".to_string(),
            sample_rate: 16000,
            channels: 1,
            bitrate: 24000,
        };
        
        let udp_config = UdpConfig {
            server: "localhost".to_string(),
            port: 1883,
            encryption: "aes-128-ctr".to_string(),
            key: vec![0u8; 16],
            nonce: vec![0u8; 16],
        };
        
        let udp_handler = UdpHandler::new(udp_config.clone());
        udp_handler.bind().await?;
        *self.udp_handler.lock().await = Some(udp_handler);
        
        let ws_config = WebSocketConfig {
            url: "ws://localhost:8080".to_string(),
            device_id: self.connection_info.lock().await.as_ref()
                .map(|info| info.mac_address.clone())
                .unwrap_or_default(),
            protocol_version: "2".to_string(),
            authorization: "Bearer test-token".to_string(),
            features: vec!["voice".to_string(), "text".to_string()],
        };
        
        let ws_bridge = WebSocketBridge::new(ws_config);
        ws_bridge.connect().await?;
        ws_bridge.send_hello(audio_params.clone()).await?;
        *self.websocket_bridge.lock().await = Some(ws_bridge);
        
        let hello_reply = serde_json::json!({
            "type": "hello",
            "version": 3,
            "session_id": session_id,
            "transport": "udp",
            "udp": {
                "server": udp_config.server,
                "port": udp_config.port,
                "encryption": udp_config.encryption,
                "key": hex::encode(&udp_config.key),
                "nonce": hex::encode(&udp_config.nonce)
            },
            "audio_params": audio_params
        });
        
        self.send_message("devices/p2p/gateway", &hello_reply).await?;
        
        info!("Hello 消息处理完成，会话ID: {}", session_id);
        Ok(())
    }
    
    async fn handle_audio_message(&mut self, json: &serde_json::Value) -> Result<()> {
        debug!("处理音频消息");
        
        if let Some(udp_handler) = self.udp_handler.lock().await.as_ref() {
            if let Some(data) = json.get("data").and_then(|v| v.as_str()) {
                if let Ok(audio_data) = hex::decode(data) {
                    let timestamp = json.get("timestamp")
                        .and_then(|v| v.as_u64())
                        .unwrap_or_else(|| {
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                        });
                    
                    udp_handler.send_audio(audio_data, timestamp).await?;
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_text_message(&mut self, json: &serde_json::Value) -> Result<()> {
        debug!("处理文本消息");
        
        if let Some(ws_bridge) = self.websocket_bridge.lock().await.as_ref() {
            if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
                let timestamp = json.get("timestamp")
                    .and_then(|v| v.as_u64())
                    .unwrap_or_else(|| {
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                    });
                
                ws_bridge.send_text(text.to_string(), timestamp).await?;
            }
        }
        
        Ok(())
    }
    
    async fn handle_mcp_message(&mut self, json: &serde_json::Value) -> Result<()> {
        debug!("处理 MCP 消息");
        
        if let Some(ws_bridge) = self.websocket_bridge.lock().await.as_ref() {
            if let Some(payload) = json.get("payload") {
                ws_bridge.send_mcp(payload.clone()).await?;
            }
        }
        
        Ok(())
    }
    
    async fn handle_goodbye_message(&mut self, _json: &serde_json::Value) -> Result<()> {
        info!("处理 Goodbye 消息");
        
        if let Some(ws_bridge) = self.websocket_bridge.lock().await.take() {
            let _ = ws_bridge.send_goodbye().await;
            let _ = ws_bridge.disconnect().await;
        }
        
        if let Some(udp_handler) = self.udp_handler.lock().await.take() {
            let _ = udp_handler.close().await;
        }
        
        Ok(())
    }
    
    async fn handle_subscribe(&mut self, subscribe_data: mqtt_protocol::SubscribeData) -> Result<()> {
        debug!("处理 SUBSCRIBE 消息: {} (QoS {})", subscribe_data.topic, subscribe_data.qos);
        self.mqtt_protocol.send_suback(subscribe_data.packet_id, subscribe_data.qos).await?;
        Ok(())
    }
    
    async fn handle_pingreq(&mut self) -> Result<()> {
        debug!("处理 PINGREQ 消息");
        self.mqtt_protocol.send_pingresp().await?;
        Ok(())
    }
    
    async fn handle_disconnect(&mut self) -> Result<()> {
        info!("处理 DISCONNECT 消息");
        *self.is_connected.lock().await = false;
        Ok(())
    }
    
    async fn send_message(&mut self, topic: &str, payload: &serde_json::Value) -> Result<()> {
        let payload_str = serde_json::to_string(payload)?;
        self.mqtt_protocol.send_publish(topic, payload_str.as_bytes(), 0).await?;
        Ok(())
    }
    
    pub async fn close(&mut self) -> Result<()> {
        info!("关闭连接: {}", self.connection_id);
        
        if let Some(ws_bridge) = self.websocket_bridge.lock().await.take() {
            let _ = ws_bridge.disconnect().await;
        }
        
        if let Some(udp_handler) = self.udp_handler.lock().await.take() {
            let _ = udp_handler.close().await;
        }
        
        *self.is_connected.lock().await = false;
        Ok(())
    }
    
    pub fn get_connection_id(&self) -> &str {
        &self.connection_id
    }
    
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.lock().await
    }
    
    pub async fn get_connection_info(&self) -> Option<ConnectionInfo> {
        self.connection_info.lock().await.clone()
    }
}

pub struct ConnectionManager {
    connections: Arc<RwLock<HashMap<String, Arc<Mutex<MqttConnection>>>>>,
    auth_manager: Arc<AuthManager>,
}

impl ConnectionManager {
    pub fn new(auth_manager: Arc<AuthManager>) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            auth_manager,
        }
    }
    
    pub async fn add_connection(&self, connection: MqttConnection) -> String {
        let connection_id = connection.get_connection_id().to_string();
        let connection = Arc::new(Mutex::new(connection));
        
        self.connections.write().await.insert(connection_id.clone(), connection);
        info!("添加连接: {}", connection_id);
        
        connection_id
    }
    
    pub async fn remove_connection(&self, connection_id: &str) -> Option<Arc<Mutex<MqttConnection>>> {
        let connection = self.connections.write().await.remove(connection_id);
        if connection.is_some() {
            info!("移除连接: {}", connection_id);
        }
        connection
    }
    
    pub async fn get_connection(&self, connection_id: &str) -> Option<Arc<Mutex<MqttConnection>>> {
        self.connections.read().await.get(connection_id).cloned()
    }
    
    pub async fn get_all_connections(&self) -> Vec<String> {
        self.connections.read().await.keys().cloned().collect()
    }
    
    pub async fn get_connection_count(&self) -> usize {
        self.connections.read().await.len()
    }
    
    pub async fn start_heartbeat_check(&self, interval_secs: u64) {
        let connections = self.connections.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                
                let mut connections_guard = connections.write().await;
                let mut expired_connections = Vec::new();
                
                for (connection_id, connection) in connections_guard.iter() {
                    let conn = connection.lock().await;
                    if let Some(info) = conn.get_connection_info().await {
                        let elapsed = info.last_activity.elapsed();
                        let timeout = Duration::from_secs((info.keep_alive_interval * 3 / 2) as u64);
                        
                        if elapsed > timeout {
                            warn!("连接心跳超时: {} (最后活动: {:?})", connection_id, elapsed);
                            expired_connections.push(connection_id.clone());
                        }
                    }
                }
                
                for connection_id in expired_connections {
                    connections_guard.remove(&connection_id);
                    info!("移除超时连接: {}", connection_id);
                }
            }
        });
    }
    
    pub async fn cleanup_expired_tokens(&self) {
        self.auth_manager.cleanup_expired_tokens();
    }
}
