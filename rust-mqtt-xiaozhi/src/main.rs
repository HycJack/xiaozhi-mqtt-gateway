use tokio::net::{TcpListener, UdpSocket};
use tokio::signal;
use tokio::time::{interval, Duration};
use std::sync::Arc;
use log::{info, error, debug};
use uuid::Uuid;
use config_manager::{ConfigManager, GatewayConfig};
use auth_manager::AuthManager;
use anyhow::{Context, Result};
use thiserror::Error;

mod connection;

use connection::{MqttConnection, ConnectionManager};

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("服务器启动错误: {0}")]
    StartupError(String),
    
    #[error("服务器运行错误: {0}")]
    RuntimeError(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
}

pub struct MqttServer {
    config: GatewayConfig,
    auth_manager: Arc<AuthManager>,
    connection_manager: Arc<ConnectionManager>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl MqttServer {
    pub fn new(config: GatewayConfig) -> Self {
        let auth_manager = Arc::new(AuthManager::new(
            config.auth.default_username.clone(),
            config.auth.default_password.clone(),
            config.auth.protocols.clone(),
        ));
        
        let connection_manager = Arc::new(ConnectionManager::new(auth_manager.clone()));
        
        Self {
            config,
            auth_manager,
            connection_manager,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    
    pub async fn start(&self) -> Result<()> {
        info!("启动 MQTT Gateway 服务器");
        
        self.is_running.store(true, std::sync::atomic::Ordering::SeqCst);
        
        let mqtt_addr = format!("0.0.0.0:{}", self.config.mqtt.port);
        let mqtt_listener = TcpListener::bind(&mqtt_addr)
            .await
            .context(format!("无法绑定到 {}", mqtt_addr))?;
        
        info!("MQTT 服务器监听: {}", mqtt_addr);
        
        let udp_addr = format!("0.0.0.0:{}", self.config.udp.port);
        let udp_socket = UdpSocket::bind(&udp_addr)
            .await
            .context(format!("无法绑定到 {}", udp_addr))?;
        
        info!("UDP 服务器监听: {}", udp_addr);
        
        self.connection_manager.start_heartbeat_check(1).await;
        
        let connection_manager = self.connection_manager.clone();
        let auth_manager = self.auth_manager.clone();
        let is_running = self.is_running.clone();
        
        tokio::spawn(async move {
            while is_running.load(std::sync::atomic::Ordering::SeqCst) {
                match mqtt_listener.accept().await {
                    Ok((socket, addr)) => {
                        info!("新客户端连接: {}", addr);
                        
                        let connection_id = Uuid::new_v4().to_string();
                        debug!("创建新连接: {} 来自 {}", connection_id, addr);
                        
                        let connection = MqttConnection::new(
                            socket,
                            connection_id.clone(),
                            auth_manager.clone(),
                        );
                        
                        // 先添加到连接管理器
                        connection_manager.add_connection(connection).await;
                        info!("添加连接: {}", connection_id);
                        
                        let conn_id = connection_id.clone();
                        let conn_mgr = connection_manager.clone();
                        
                        tokio::spawn(async move {
                            // 从管理器获取连接
                            if let Some(connection) = conn_mgr.get_connection(&conn_id).await {
                                let mut conn = connection.lock().await;
                                if let Err(e) = conn.handle_connection().await {
                                    error!("连接处理错误 {}: {:?}", conn_id, e);
                                }
                                
                                conn_mgr.remove_connection(&conn_id).await;
                                info!("移除连接: {}", conn_id);
                                if let Err(e) = conn.close().await {
                                    error!("关闭连接错误 {}: {:?}", conn_id, e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!("接受连接错误: {}", e);
                        // 短暂暂停后继续，避免错误导致 CPU 占用过高
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        
        let udp_socket_arc = Arc::new(udp_socket);
        let connection_manager_udp = self.connection_manager.clone();
        let is_running_udp = self.is_running.clone();
        
        tokio::spawn(async move {
            let mut buffer = vec![0u8; 8192];
            
            while is_running_udp.load(std::sync::atomic::Ordering::SeqCst) {
                match udp_socket_arc.recv_from(&mut buffer).await {
                    Ok((len, addr)) => {
                        debug!("收到 UDP 数据: {} 字节 来自 {}", len, addr);
                        
                        let data = buffer[..len].to_vec();
                        let conn_mgr = connection_manager_udp.clone();
                        
                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_udp_message(data, addr, conn_mgr).await {
                                error!("UDP 消息处理错误: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("UDP 接收错误: {}", e);
                        // 短暂暂停后继续，避免错误导致 CPU 占用过高
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        
        let connection_manager_cleanup = self.connection_manager.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                debug!("执行连接清理任务");
                connection_manager_cleanup.cleanup_expired_tokens().await;
                debug!("连接清理任务完成");
            }
        });
        
        info!("服务器启动完成");
        Ok(())
    }
    
    async fn handle_udp_message(
        data: Vec<u8>,
        addr: std::net::SocketAddr,
        connection_manager: Arc<ConnectionManager>,
    ) -> Result<()> {
        debug!("处理 UDP 消息: {} 字节 来自 {}", data.len(), addr);
        
        if data.len() >= 16 {
            let timestamp = u64::from_be_bytes([
                data[8], data[9], data[10], data[11],
                data[12], data[13], data[14], data[15]
            ]);
            
            let connections = connection_manager.get_all_connections().await;
            for connection_id in connections {
                if let Some(connection) = connection_manager.get_connection(&connection_id).await {
                    let conn: tokio::sync::MutexGuard<connection::MqttConnection> = connection.lock().await;
                    if let Some(info) = conn.get_connection_info().await {
                        debug!("转发 UDP 数据到连接: {}", connection_id);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        info!("停止 MQTT Gateway 服务器");
        
        self.is_running.store(false, std::sync::atomic::Ordering::SeqCst);
        
        let connections = self.connection_manager.get_all_connections().await;
        for connection_id in connections {
            if let Some(connection) = self.connection_manager.remove_connection(&connection_id).await {
                let mut conn: tokio::sync::MutexGuard<connection::MqttConnection> = connection.lock().await;
                let _ = conn.close().await;
            }
        }
        
        info!("服务器已停止");
        Ok(())
    }
    
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }
    
    pub fn get_connection_count(&self) -> usize {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.connection_manager.get_connection_count().await
            })
        })
    }
}

pub async fn run_server() -> Result<()> {
    env_logger::init();
    
    info!("XiaoZhi MQTT Gateway - Rust 版本");
    
    info!("加载配置...");
    let config = ConfigManager::get_config();
    info!("配置加载成功: {:?}", config);
    
    info!("创建服务器实例...");
    let server = MqttServer::new(config);
    
    info!("启动服务器...");
    match server.start().await {
        Ok(_) => {
            info!("服务器启动成功");
        }
        Err(e) => {
            error!("服务器启动失败: {:?}", e);
            return Err(e);
        }
    }
    
    info!("等待 Ctrl+C 信号...");
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("收到 Ctrl+C 信号");
        }
    }
    
    info!("停止服务器...");
    match server.stop().await {
        Ok(_) => {
            info!("服务器已停止");
        }
        Err(e) => {
            error!("服务器停止失败: {:?}", e);
        }
    }
    
    Ok(())
}

fn main() -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_server())
}
