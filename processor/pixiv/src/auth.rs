use anyhow::{Result, anyhow};
use common::get_env_var;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::constants::{CLIENT_ID, CLIENT_SECRET, PIXIV_UA};
use crate::models::{PixivTokenError, PixivTokenResponse};

/// 令牌缓存结构
#[derive(Debug, Clone)]
pub struct TokenCache {
    access_token: Option<String>,
    expires_at: Option<u64>,
}

impl TokenCache {
    pub fn new() -> Self {
        Self {
            access_token: None,
            expires_at: None,
        }
    }

    pub fn is_valid(&self) -> bool {
        if let (Some(_), Some(expires_at)) = (&self.access_token, self.expires_at) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            // 提前5分钟过期，避免在请求过程中过期
            now + 300 < expires_at
        } else {
            false
        }
    }

    pub fn set_token(&mut self, token: String, expires_in: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.access_token = Some(token);
        self.expires_at = Some(now + expires_in);
    }

    pub fn get_token(&self) -> Option<&str> {
        if self.is_valid() {
            self.access_token.as_deref()
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.access_token = None;
        self.expires_at = None;
    }
}

// 全局令牌缓存
static TOKEN_CACHE: OnceLock<Mutex<TokenCache>> = OnceLock::new();

fn get_token_cache() -> &'static Mutex<TokenCache> {
    TOKEN_CACHE.get_or_init(|| Mutex::new(TokenCache::new()))
}

/// 获取访问令牌（带缓存版本）
pub async fn get_access_token() -> Result<String> {
    // 首先检查缓存中的令牌是否有效
    {
        let cache = get_token_cache().lock().unwrap();
        if let Some(token) = cache.get_token() {
            log::debug!("Using cached Pixiv access token");
            return Ok(token.to_string());
        }
    }

    // 缓存中没有有效令牌，需要刷新
    log::debug!("Cached token invalid or missing, refreshing Pixiv access token");

    // 从环境变量获取 refresh_token
    let refresh_token = get_env_var("PIXIV_REFRESH_TOKEN")
        .ok_or_else(|| anyhow!("PIXIV_REFRESH_TOKEN environment variable not set"))?;

    refresh_access_token(&refresh_token).await
}

/// 获取访问令牌并在失败时清除缓存重试
pub async fn get_access_token_with_retry() -> Result<String> {
    match get_access_token().await {
        Ok(token) => Ok(token),
        Err(e) => {
            log::warn!(
                "Failed to get Pixiv access token, clearing cache and retrying: {}",
                e
            );
            // 清除缓存的令牌
            {
                let mut cache = get_token_cache().lock().unwrap();
                cache.clear();
            }

            // 重试一次
            get_access_token().await
        }
    }
}

/// 刷新访问令牌
async fn refresh_access_token(refresh_token: &str) -> Result<String> {
    let client = reqwest::Client::new();

    let form_data = [
        ("client_id", CLIENT_ID),
        ("client_secret", CLIENT_SECRET),
        ("get_secure_url", "1"),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];

    let response = client
        .post("https://oauth.secure.pixiv.net/auth/token")
        .header("User-Agent", PIXIV_UA)
        .header("Accept-Language", "en-US,en;q=0.9")
        .form(&form_data)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to refresh Pixiv token: HTTP {}",
            response.status()
        ));
    }

    let text = response.text().await?;
    log::trace!("Pixiv token response: {}", text);

    // 尝试解析成功响应
    match serde_json::from_str::<PixivTokenResponse>(&text) {
        std::result::Result::Ok(token_response) => {
            log::debug!("Pixiv token refresh successful");

            // 更新缓存
            {
                let mut cache = get_token_cache().lock().unwrap();
                cache.set_token(
                    token_response.access_token.clone(),
                    token_response.expires_in,
                );
            }

            std::result::Result::Ok(token_response.access_token)
        }
        std::result::Result::Err(_) => {
            // 尝试解析错误响应
            match serde_json::from_str::<PixivTokenError>(&text) {
                std::result::Result::Ok(error_response) => Err(anyhow!(
                    "Pixiv token refresh failed: {} - {}",
                    error_response.error,
                    error_response.error_description.unwrap_or_default()
                )),
                std::result::Result::Err(_) => {
                    Err(anyhow!("Failed to parse Pixiv token response: {}", text))
                }
            }
        }
    }
}
