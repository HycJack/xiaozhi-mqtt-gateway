use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use thiserror::Error;
use regex::Regex;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("认证失败: {0}")]
    AuthenticationFailed(String),
    
    #[error("无效的客户端ID: {0}")]
    InvalidClientId(String),
    
    #[error("无效的MAC地址: {0}")]
    InvalidMacAddress(String),
    
    #[error("认证协议不支持: {0}")]
    UnsupportedProtocol(String),
    
    #[error("令牌无效: {0}")]
    InvalidToken(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCredentials {
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    pub group_id: String,
    pub mac_address: String,
    pub uuid: Option<String>,
    pub user_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
    pub mac_address: String,
    pub group_id: String,
    pub uuid: Option<String>,
    pub expires_at: Option<u64>,
    pub permissions: Vec<String>,
}

lazy_static! {
    static ref MAC_ADDRESS_REGEX: Regex = Regex::new(
        r"^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$"
    ).expect("无效的MAC地址正则表达式");
    
    static ref CLIENT_ID_REGEX: Regex = Regex::new(
        r"^[A-Za-z0-9_]+@@@([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})(@@@[A-Za-z0-9_-]+)?$"
    ).expect("无效的客户端ID正则表达式");
    
    static ref TOKEN_STORE: RwLock<HashMap<String, TokenData>> = RwLock::new(HashMap::new());
}

pub struct AuthManager {
    default_username: Option<String>,
    default_password: Option<String>,
    enabled_protocols: Vec<String>,
}

impl AuthManager {
    pub fn new(default_username: Option<String>, default_password: Option<String>, enabled_protocols: Vec<String>) -> Self {
        Self {
            default_username,
            default_password,
            enabled_protocols,
        }
    }
    
    pub fn authenticate(&self, credentials: &AuthCredentials) -> Result<AuthResult> {
        if !self.is_valid_client_id(&credentials.client_id) {
            return Err(AuthError::InvalidClientId(credentials.client_id.clone()).into());
        }
        
        let parts: Vec<&str> = credentials.client_id.split("@@@").collect();
        let group_id = parts[0].to_string();
        let mac_address = parts[1].replace("_", ":");
        
        if !self.is_valid_mac_address(&mac_address) {
            return Err(AuthError::InvalidMacAddress(mac_address).into());
        }
        
        let uuid = if parts.len() == 3 {
            Some(parts[2].to_string())
        } else {
            None
        };
        
        if let (Some(username), Some(password)) = (&credentials.username, &credentials.password) {
            if username.starts_with("oauth2:") {
                return self.authenticate_oauth2(username, password, &mac_address, &group_id, uuid.as_deref());
            } else if username.starts_with("jwt:") {
                return self.authenticate_jwt(username, password, &mac_address, &group_id, uuid.as_deref());
            } else if username.starts_with("token:") {
                return self.authenticate_token(username, password, &mac_address, &group_id, uuid.as_deref());
            }
        }
        
        self.authenticate_default(credentials, &mac_address, &group_id, uuid.as_deref())
    }
    
    fn authenticate_default(&self, credentials: &AuthCredentials, mac_address: &str, group_id: &str, uuid: Option<&str>) -> Result<AuthResult> {
        if let (Some(default_username), Some(default_password)) = (&self.default_username, &self.default_password) {
            if credentials.username.as_ref() != Some(default_username) ||
               credentials.password.as_ref() != Some(default_password) {
                return Err(AuthError::AuthenticationFailed("默认认证失败".to_string()).into());
            }
        }
        
        Ok(AuthResult {
            group_id: group_id.to_string(),
            mac_address: mac_address.to_string(),
            uuid: uuid.map(|u| u.to_string()),
            user_data: None,
        })
    }
    
    fn authenticate_oauth2(&self, username: &str, password: &str, mac_address: &str, group_id: &str, uuid: Option<&str>) -> Result<AuthResult> {
        let token = username.strip_prefix("oauth2:")
            .ok_or_else(|| AuthError::AuthenticationFailed("无效的OAuth2令牌格式".to_string()))?;
        
        if !self.validate_oauth2_token(token, password, mac_address) {
            return Err(AuthError::AuthenticationFailed("OAuth2令牌验证失败".to_string()).into());
        }
        
        Ok(AuthResult {
            group_id: group_id.to_string(),
            mac_address: mac_address.to_string(),
            uuid: uuid.map(|u| u.to_string()),
            user_data: Some(serde_json::json!({
                "auth_type": "oauth2",
                "access_token": token,
                "expires_in": 3600
            })),
        })
    }
    
    fn authenticate_jwt(&self, username: &str, password: &str, mac_address: &str, group_id: &str, uuid: Option<&str>) -> Result<AuthResult> {
        let token = username.strip_prefix("jwt:")
            .ok_or_else(|| AuthError::AuthenticationFailed("无效的JWT令牌格式".to_string()))?;
        
        if !self.validate_jwt_token(token, password, mac_address) {
            return Err(AuthError::AuthenticationFailed("JWT令牌验证失败".to_string()).into());
        }
        
        Ok(AuthResult {
            group_id: group_id.to_string(),
            mac_address: mac_address.to_string(),
            uuid: uuid.map(|u| u.to_string()),
            user_data: Some(serde_json::json!({
                "auth_type": "jwt",
                "access_token": token,
                "expires_in": 3600,
                "permissions": ["voice", "text"]
            })),
        })
    }
    
    fn authenticate_token(&self, username: &str, password: &str, mac_address: &str, group_id: &str, uuid: Option<&str>) -> Result<AuthResult> {
        let token = username.strip_prefix("token:")
            .ok_or_else(|| AuthError::AuthenticationFailed("无效的令牌格式".to_string()))?;
        
        let token_store = TOKEN_STORE.read().unwrap();
        let token_data = token_store.get(token)
            .ok_or_else(|| AuthError::InvalidToken("令牌不存在".to_string()))?;
        
        if token_data.mac_address != mac_address {
            return Err(AuthError::AuthenticationFailed("令牌与设备不匹配".to_string()).into());
        }
        
        if let Some(expires_at) = token_data.expires_at {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            if current_time > expires_at {
                return Err(AuthError::InvalidToken("令牌已过期".to_string()).into());
            }
        }
        
        Ok(AuthResult {
            group_id: token_data.group_id.clone(),
            mac_address: mac_address.to_string(),
            uuid: uuid.map(|u| u.to_string()),
            user_data: Some(serde_json::json!({
                "auth_type": "token",
                "permissions": token_data.permissions
            })),
        })
    }
    
    fn validate_oauth2_token(&self, _token: &str, _password: &str, _mac_address: &str) -> bool {
        true
    }
    
    fn validate_jwt_token(&self, _token: &str, _password: &str, _mac_address: &str) -> bool {
        true
    }
    
    pub fn generate_token(&self, mac_address: &str, group_id: &str, uuid: Option<String>, expires_in: Option<u64>) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        
        let expires_at = expires_in.map(|duration| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() + duration
        });
        
        let token_data = TokenData {
            mac_address: mac_address.to_string(),
            group_id: group_id.to_string(),
            uuid,
            expires_at,
            permissions: vec!["voice".to_string(), "text".to_string()],
        };
        
        TOKEN_STORE.write().unwrap().insert(token.clone(), token_data);
        log::info!("生成令牌: {} for {}", token, mac_address);
        
        token
    }
    
    pub fn revoke_token(&self, token: &str) -> Result<()> {
        TOKEN_STORE.write().unwrap()
            .remove(token)
            .ok_or_else(|| AuthError::InvalidToken("令牌不存在".to_string()))?;
        
        log::info!("撤销令牌: {}", token);
        Ok(())
    }
    
    pub fn is_valid_client_id(&self, client_id: &str) -> bool {
        CLIENT_ID_REGEX.is_match(client_id)
    }
    
    pub fn is_valid_mac_address(&self, mac_address: &str) -> bool {
        MAC_ADDRESS_REGEX.is_match(mac_address)
    }
    
    pub fn is_protocol_enabled(&self, protocol: &str) -> bool {
        self.enabled_protocols.contains(&protocol.to_string())
    }
    
    pub fn parse_client_id(&self, client_id: &str) -> Result<(String, String, Option<String>)> {
        if !self.is_valid_client_id(client_id) {
            return Err(AuthError::InvalidClientId(client_id.to_string()).into());
        }
        
        let parts: Vec<&str> = client_id.split("@@@").collect();
        let group_id = parts[0].to_string();
        let mac_address = parts[1].replace("_", ":");
        let uuid = if parts.len() == 3 {
            Some(parts[2].to_string())
        } else {
            None
        };
        
        Ok((group_id, mac_address, uuid))
    }
    
    pub fn cleanup_expired_tokens(&self) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut token_store = TOKEN_STORE.write().unwrap();
        let mut expired_tokens = Vec::new();
        
        for (token, token_data) in token_store.iter() {
            if let Some(expires_at) = token_data.expires_at {
                if current_time > expires_at {
                    expired_tokens.push(token.clone());
                }
            }
        }
        
        for token in expired_tokens {
            token_store.remove(&token);
            log::info!("清理过期令牌: {}", token);
        }
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new(None, None, vec!["default".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_valid_client_id() {
        let auth_manager = AuthManager::default();
        assert!(auth_manager.is_valid_client_id("GID_test@@@00:11:22:33:44:55"));
        assert!(auth_manager.is_valid_client_id("GID_test@@@00:11:22:33:44:55@@@uuid123"));
        assert!(!auth_manager.is_valid_client_id("invalid_client_id"));
    }
    
    #[test]
    fn test_valid_mac_address() {
        let auth_manager = AuthManager::default();
        assert!(auth_manager.is_valid_mac_address("00:11:22:33:44:55"));
        assert!(auth_manager.is_valid_mac_address("00-11-22-33-44-55"));
        assert!(!auth_manager.is_valid_mac_address("invalid_mac"));
    }
    
    #[test]
    fn test_parse_client_id() {
        let auth_manager = AuthManager::default();
        let (group_id, mac_address, uuid) = auth_manager.parse_client_id("GID_test@@@00:11:22:33:44:55@@@uuid123").unwrap();
        assert_eq!(group_id, "GID_test");
        assert_eq!(mac_address, "00:11:22:33:44:55");
        assert_eq!(uuid, Some("uuid123".to_string()));
    }
    
    #[test]
    fn test_generate_token() {
        let auth_manager = AuthManager::default();
        let token = auth_manager.generate_token("00:11:22:33:44:55", "GID_test", None, None);
        assert!(!token.is_empty());
    }
}
