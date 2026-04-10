use tokio::net::UdpSocket;
use bytes::{BytesMut, Bytes};
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{debug, info};
use anyhow::{Result, Context};

pub struct UdpZeroCopyHandler {
    socket: Arc<UdpSocket>,
    buffer_pool: Arc<Mutex<Vec<BytesMut>>>,
    buffer_size: usize,
    max_pool_size: usize,
}

impl UdpZeroCopyHandler {
    pub fn new(socket: Arc<UdpSocket>, buffer_size: usize, max_pool_size: usize) -> Self {
        Self {
            socket,
            buffer_pool: Arc::new(Mutex::new(Vec::with_capacity(1024))),
            buffer_size,
            max_pool_size,
        }
    }

    // 获取缓冲区
    async fn get_buffer(&self) -> BytesMut {
        let mut pool = self.buffer_pool.lock().await;
        if let Some(buffer) = pool.pop() {
            buffer
        } else {
            BytesMut::with_capacity(self.buffer_size)
        }
    }

    // 归还缓冲区
    async fn return_buffer(&self, mut buffer: BytesMut) {
        buffer.clear();
        let mut pool = self.buffer_pool.lock().await;
        if pool.len() < self.max_pool_size {
            pool.push(buffer);
        }
    }

    // 零拷贝接收
    pub async fn recv_zero_copy(&self) -> Result<(Bytes, std::net::SocketAddr)> {
        let mut buffer = self.get_buffer().await;
        buffer.resize(self.buffer_size, 0);
        
        let (len, addr) = self.socket.recv_from(&mut buffer)
            .await
            .context("UDP 接收失败")?;
        
        buffer.truncate(len);
        let data = buffer.freeze();
        
        Ok((data, addr))
    }

    // 零拷贝发送
    pub async fn send_zero_copy(&self, data: &[u8], addr: &std::net::SocketAddr) -> Result<usize> {
        self.socket.send_to(data, addr)
            .await
            .context("UDP 发送失败")
    }

    // 处理完数据后归还缓冲区
    pub async fn process_completed(&self, buffer: BytesMut) {
        self.return_buffer(buffer).await;
    }

    // 批量接收
    pub async fn recv_batch(&self, batch_size: usize) -> Result<Vec<(Bytes, std::net::SocketAddr)>> {
        let mut results = Vec::with_capacity(batch_size);
        
        for _ in 0..batch_size {
            match self.recv_zero_copy().await {
                Ok((data, addr)) => results.push((data, addr)),
                Err(e) => {
                    debug!("UDP 接收错误: {}", e);
                    break;
                }
            }
        }
        
        Ok(results)
    }

    // 获取当前缓冲区池大小
    pub async fn get_pool_size(&self) -> usize {
        let pool = self.buffer_pool.lock().await;
        pool.len()
    }

    // 清理缓冲区池
    pub async fn clear_pool(&self) {
        let mut pool = self.buffer_pool.lock().await;
        pool.clear();
        info!("UDP 缓冲区池已清理");
    }
}