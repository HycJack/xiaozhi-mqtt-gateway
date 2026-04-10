use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use thiserror::Error;
use std::path::Path;
use std::fs;
use lazy_static::lazy_static;
use std::sync::RwLock;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("配置文件错误: {0}")]
    FileError(String),
    
    #[error("配置解析错误: {0}")]
    ParseError(String),
    
    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON 解析错误: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("TOML 解析错误: {0}")]
    TomlError(#[from] toml::de::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    pub port: u16,
    pub max_connections: usize,
    pub max_payload_size: usize,
    pub keep_alive_timeout: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdpConfig {
    pub port: u16,
    pub max_packet_size: usize,
    pub encryption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub url: String,
    pub reconnect_interval: u64,
    pub max_reconnect_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub protocols: Vec<String>,
    pub default_username: Option<String>,
    pub default_password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub mqtt: MqttConfig,
    pub udp: UdpConfig,
    pub websocket: WebSocketConfig,
    pub auth: AuthConfig,
    pub logging: LoggingConfig,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            mqtt: MqttConfig {
                port: 1883,
                max_connections: 1000,
                max_payload_size: 8192,
                keep_alive_timeout: 60,
            },
            udp: UdpConfig {
                port: 1884,
                max_packet_size: 8192,
                encryption: "aes-128-ctr".to_string(),
            },
            websocket: WebSocketConfig {
                url: "ws://localhost:8080".to_string(),
                reconnect_interval: 5000,
                max_reconnect_attempts: 10,
            },
            auth: AuthConfig {
                enabled: true,
                protocols: vec!["default".to_string()],
                default_username: None,
                default_password: None,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: None,
            },
        }
    }
}

lazy_static! {
    static ref CONFIG: RwLock<GatewayConfig> = RwLock::new(GatewayConfig::default());
}

pub struct ConfigManager;

impl ConfigManager {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<()> {
        let content = fs::read_to_string(path.as_ref())
            .context("读取配置文件失败")?;
        
        let config: GatewayConfig = if path.as_ref().extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
            .as_deref() == Some("toml") {
            toml::from_str(&content)?
        } else {
            serde_json::from_str(&content)?
        };
        
        *CONFIG.write().unwrap() = config;
        log::info!("配置文件加载成功: {}", path.as_ref().display());
        Ok(())
    }
    
    pub fn load_from_json(json: &str) -> Result<()> {
        let config: GatewayConfig = serde_json::from_str(json)?;
        *CONFIG.write().unwrap() = config;
        log::info!("从 JSON 加载配置成功");
        Ok(())
    }
    
    pub fn get_config() -> GatewayConfig {
        CONFIG.read().unwrap().clone()
    }
    
    pub fn get_mqtt_config() -> MqttConfig {
        CONFIG.read().unwrap().mqtt.clone()
    }
    
    pub fn get_udp_config() -> UdpConfig {
        CONFIG.read().unwrap().udp.clone()
    }
    
    pub fn get_websocket_config() -> WebSocketConfig {
        CONFIG.read().unwrap().websocket.clone()
    }
    
    pub fn get_auth_config() -> AuthConfig {
        CONFIG.read().unwrap().auth.clone()
    }
    
    pub fn get_logging_config() -> LoggingConfig {
        CONFIG.read().unwrap().logging.clone()
    }
    
    pub fn set_config(config: GatewayConfig) {
        *CONFIG.write().unwrap() = config;
        log::info!("配置已更新");
    }
    
    pub fn set_mqtt_config(config: MqttConfig) {
        CONFIG.write().unwrap().mqtt = config;
        log::info!("MQTT 配置已更新");
    }
    
    pub fn set_udp_config(config: UdpConfig) {
        CONFIG.write().unwrap().udp = config;
        log::info!("UDP 配置已更新");
    }
    
    pub fn set_websocket_config(config: WebSocketConfig) {
        CONFIG.write().unwrap().websocket = config;
        log::info!("WebSocket 配置已更新");
    }
    
    pub fn set_auth_config(config: AuthConfig) {
        CONFIG.write().unwrap().auth = config;
        log::info!("认证配置已更新");
    }
    
    pub fn save_to_file<P: AsRef<Path>>(path: P) -> Result<()> {
        let config = CONFIG.read().unwrap();
        let content = if path.as_ref().extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
            .as_deref() == Some("toml") {
            toml::to_string(&*config)?
        } else {
            serde_json::to_string_pretty(&*config)?
        };
        
        fs::write(path.as_ref(), content)
            .context("写入配置文件失败")?;
        
        log::info!("配置文件保存成功: {}", path.as_ref().display());
        Ok(())
    }
    
    pub fn reload_from_file<P: AsRef<Path>>(path: P) -> Result<()> {
        Self::load_from_file(path)
    }
    
    pub fn get_value<T: serde::de::DeserializeOwned>(key: &str) -> Result<T> {
        let config = CONFIG.read().unwrap();
        let value = serde_json::to_value(&*config)?;
        
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &value;
        
        for part in parts {
            current = current.get(part)
                .ok_or_else(|| ConfigError::ParseError(format!("配置键不存在: {}", key)))?;
        }
        
        serde_json::from_value(current.clone())
            .context("配置值解析失败")
    }
    
    pub fn set_value<T: serde::Serialize>(key: &str, value: T) -> Result<()> {
        let config = CONFIG.read().unwrap().clone();
        let mut config_value = serde_json::to_value(&config)?;
        
        // 在循环外序列化 value，避免移动错误
        let value_json = serde_json::to_value(&value)?;
        
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &mut config_value;
        
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                *current.get_mut(part).ok_or_else(|| {
                    ConfigError::ParseError(format!("配置键不存在: {}", key))
                })? = value_json.clone();
            } else {
                current = current.get_mut(part).ok_or_else(|| {
                    ConfigError::ParseError(format!("配置键不存在: {}", key))
                })?;
            }
        }
        
        let updated_config: GatewayConfig = serde_json::from_value(config_value)?;
        *CONFIG.write().unwrap() = updated_config;
        log::info!("配置值已更新: {}", key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = GatewayConfig::default();
        assert_eq!(config.mqtt.port, 1883);
        assert_eq!(config.udp.port, 1883);
    }
    
    #[test]
    fn test_config_manager() {
        let config = ConfigManager::get_config();
        assert_eq!(config.mqtt.port, 1883);
    }
}
