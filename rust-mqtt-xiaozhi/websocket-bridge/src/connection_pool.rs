use tokio_tungstenite::{WebSocketStream, tungstenite::Message, MaybeTlsStream};
use tokio::net::TcpStream;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use std::collections::HashMap;
use uuid::Uuid;
use std::time::{Instant, Duration};
use log::{info, debug, error};
use anyhow::Result;

pub struct WebSocketPool {
    connections: Arc<Mutex<HashMap<String, Arc<Mutex<WebSocketConnection>>>>>,
    max_connections: usize,
    semaphore: Arc<Semaphore>,
    idle_timeout: Duration,
}

pub struct WebSocketConnection {
    id: String,
    stream: Arc<Mutex<Option<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    last_activity: Arc<Mutex<Instant>>,
    is_connected: Arc<Mutex<bool>>,
}

impl WebSocketPool {
    pub fn new(max_connections: usize, idle_timeout: Duration) -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            max_connections,
            semaphore: Arc::new(Semaphore::new(max_connections)),
            idle_timeout,
        }
    }

    // 获取连接
    pub async fn get_connection(&self, key: &str) -> Option<Arc<Mutex<WebSocketConnection>>> {
        let connections = self.connections.lock().await;
        connections.get(key).cloned()
    }

    // 添加连接
    pub async fn add_connection(&self, stream: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Result<String> {
        let permit = self.semaphore.acquire().await.map_err(|e| {
            anyhow::anyhow!("获取连接许可失败: {}", e)
        })?;

        let id = Uuid::new_v4().to_string();
        let connection = Arc::new(Mutex::new(WebSocketConnection {
            id: id.clone(),
            stream: Arc::new(Mutex::new(Some(stream))),
            last_activity: Arc::new(Mutex::new(Instant::now())),
            is_connected: Arc::new(Mutex::new(true)),
        }));

        {
            let mut connections = self.connections.lock().await;
            connections.insert(id.clone(), connection);
        }

        // 自动释放许可
        let _permit = permit;
        drop(_permit);

        Ok(id)
    }

    // 移除连接
    pub async fn remove_connection(&self, id: &str) {
        let mut connections = self.connections.lock().await;
        if connections.remove(id).is_some() {
            debug!("移除 WebSocket 连接: {}", id);
        }
    }

    // 清理空闲连接
    pub async fn cleanup_idle_connections(&self) {
        let now = Instant::now();
        let mut connections = self.connections.lock().await;
        let mut to_remove = Vec::new();

        for (id, connection) in connections.iter() {
            let conn = connection.lock().await;
            let last_activity = *conn.last_activity.lock().await;
            if now.duration_since(last_activity) > self.idle_timeout {
                to_remove.push(id.clone());
            }
        }

        for id in to_remove {
            if let Some(connection) = connections.remove(&id) {
                info!("清理空闲 WebSocket 连接: {}", id);
                let conn = connection.lock().await;
                let mut is_connected = conn.is_connected.lock().await;
                *is_connected = false;
            }
        }
    }

    // 获取连接数
    pub async fn get_connection_count(&self) -> usize {
        let connections = self.connections.lock().await;
        connections.len()
    }

    // 清空连接池
    pub async fn clear(&self) {
        let mut connections = self.connections.lock().await;
        connections.clear();
        info!("WebSocket 连接池已清空");
    }
}

impl WebSocketConnection {
    // 发送消息
    pub async fn send(&self, message: Message) -> Result<()> {
        let mut stream = self.stream.lock().await;
        if let Some(ws_stream) = stream.as_mut() {
            ws_stream.send(message).await?;
            *self.last_activity.lock().await = Instant::now();
        }
        Ok(())
    }

    // 接收消息
    pub async fn recv(&self) -> Result<Option<Message>> {
        let mut stream = self.stream.lock().await;
        if let Some(ws_stream) = stream.as_mut() {
            match ws_stream.next().await {
                Some(Ok(message)) => {
                    *self.last_activity.lock().await = Instant::now();
                    Ok(Some(message))
                },
                Some(Err(e)) => {
                    error!("WebSocket 接收错误: {}", e);
                    *self.is_connected.lock().await = false;
                    Ok(None)
                },
                None => {
                    *self.is_connected.lock().await = false;
                    Ok(None)
                },
            }
        } else {
            Ok(None)
        }
    }

    // 关闭连接
    pub async fn close(&self) -> Result<()> {
        let mut stream = self.stream.lock().await;
        if let Some(ws_stream) = stream.as_mut().take() {
            ws_stream.close(None).await?;
            *self.is_connected.lock().await = false;
        }
        Ok(())
    }

    // 检查是否连接
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.lock().await
    }

    // 获取最后活动时间
    pub async fn last_activity(&self) -> Instant {
        *self.last_activity.lock().await
    }

    // 获取连接 ID
    pub fn id(&self) -> &str {
        &self.id
    }
}