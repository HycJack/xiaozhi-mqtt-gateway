use serde::{Serialize, Deserialize};
use bytes::{Bytes, BytesMut};
use log::debug;

// 预分配缓冲区
thread_local! {
    static SERIALIZATION_BUFFER: std::cell::RefCell<BytesMut> = std::cell::RefCell::new(BytesMut::with_capacity(4096));
}

// 优化的序列化
pub fn serialize_message<T: Serialize>(message: &T) -> Result<Bytes, serde_json::Error> {
    SERIALIZATION_BUFFER.with(|buf| {
        let mut buffer = buf.borrow_mut();
        buffer.clear();
        
        let json_string = serde_json::to_string(message)?;
        buffer.extend_from_slice(json_string.as_bytes());
        Ok(buffer.split().freeze())
    })
}

// 优化的反序列化
pub fn deserialize_message<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T, serde_json::Error> {
    serde_json::from_slice(data)
}

// 批量序列化
pub fn serialize_batch<T: Serialize>(messages: &[T]) -> Result<Vec<Bytes>, serde_json::Error> {
    let mut results = Vec::with_capacity(messages.len());
    for message in messages {
        results.push(serialize_message(message)?);
    }
    Ok(results)
}

// 序列化性能统计
pub struct SerializationStats {
    pub serialize_calls: usize,
    pub deserialize_calls: usize,
    pub serialize_bytes: usize,
    pub deserialize_bytes: usize,
}

impl SerializationStats {
    pub fn new() -> Self {
        Self {
            serialize_calls: 0,
            deserialize_calls: 0,
            serialize_bytes: 0,
            deserialize_bytes: 0,
        }
    }
    
    pub fn record_serialize(&mut self, bytes: usize) {
        self.serialize_calls += 1;
        self.serialize_bytes += bytes;
    }
    
    pub fn record_deserialize(&mut self, bytes: usize) {
        self.deserialize_calls += 1;
        self.deserialize_bytes += bytes;
    }
    
    pub fn log_stats(&self) {
        debug!(
            "序列化统计: 序列化 {} 次 ({} 字节), 反序列化 {} 次 ({} 字节)",
            self.serialize_calls,
            self.serialize_bytes,
            self.deserialize_calls,
            self.deserialize_bytes
        );
    }
}