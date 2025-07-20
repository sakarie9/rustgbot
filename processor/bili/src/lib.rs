//! BiliBili短链接处理模块
//!
//! 这个模块提供了处理BiliBili (b23.tv) 短链接重定向的功能。

use anyhow::{Result, anyhow};
use log::info;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use url::Url;

// 全局缓存，存储 b23 短链接到重定向目标的映射
static B23_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn get_b23_cache() -> &'static Mutex<HashMap<String, String>> {
    B23_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 获取 b23.tv 短链接的重定向目标 URL（带缓存）
pub async fn get_b23_redirect(short_url: &str) -> Result<String> {
    // 首先检查缓存
    {
        let cache = get_b23_cache().lock().unwrap();
        if let Some(cached_url) = cache.get(short_url) {
            info!("Cache hit for {} -> {}", short_url, cached_url);
            return Ok(cached_url.clone());
        }
    }

    // 缓存中没有，进行网络请求
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none()) // 禁用自动重定向
        .build()?;

    let response = client.get(short_url).send().await?;

    // 检查是否是重定向状态码 (3xx)
    if response.status().is_redirection() {
        if let Some(location) = response.headers().get("location") {
            let location_str = location
                .to_str()
                .map_err(|e| anyhow!("无法解析 Location 头: {}", e))?;

            // 如果是 B 站链接，清理追踪参数
            let clean_url = if location_str.contains("bilibili.com") {
                clean_bilibili_url(location_str)?
            } else {
                location_str.to_string()
            };

            // 将结果存入缓存
            {
                let mut cache = get_b23_cache().lock().unwrap();
                cache.insert(short_url.to_string(), clean_url.clone());
            }

            Ok(clean_url)
        } else {
            Err(anyhow!("响应中没有找到 Location 头"))
        }
    } else {
        Err(anyhow!(
            "期望重定向响应，但收到状态码: {}",
            response.status()
        ))
    }
}

/// 清理 B 站 URL 中的所有查询参数，返回纯净的 URL
pub fn clean_bilibili_url(url_str: &str) -> Result<String> {
    let mut url = Url::parse(url_str)?;

    // 清空所有查询参数
    url.set_query(None);

    Ok(url.to_string())
}

/// 清空 b23 缓存
#[allow(dead_code)]
pub fn clear_b23_cache() {
    let mut cache = get_b23_cache().lock().unwrap();
    cache.clear();
}

/// 获取缓存中的条目数量
#[allow(dead_code)]
pub fn get_cache_size() -> usize {
    let cache = get_b23_cache().lock().unwrap();
    cache.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_b23_redirect() {
        // 注意：这个测试需要网络连接
        let result = get_b23_redirect("https://b23.tv/YiEAeDi").await;
        match result {
            Ok(location) => {
                println!("重定向到: {}", location);
                assert!(location.contains("bilibili.com"));
                // 检查是否已清理追踪参数
                assert!(!location.contains("buvid"));
                assert!(!location.contains("share_from"));
                assert!(!location.contains("spmid"));
            }
            Err(e) => {
                panic!("获取 b23 重定向失败: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_b23_cache() {
        // 清空缓存
        clear_b23_cache();
        assert_eq!(get_cache_size(), 0);

        // 第一次请求（实际网络请求）
        let url = "https://b23.tv/YiEAeDi";
        let result1 = get_b23_redirect(url).await;

        if let Ok(location1) = result1 {
            // 检查缓存中有了一个条目
            assert_eq!(get_cache_size(), 1);

            // 第二次请求（应该从缓存获取）
            let result2 = get_b23_redirect(url).await;
            if let Ok(location2) = result2 {
                // 两次结果应该相同
                assert_eq!(location1, location2);
                // 缓存大小仍然是 1
                assert_eq!(get_cache_size(), 1);
                println!("缓存测试通过: {}", location2);
            } else {
                panic!("第二次请求失败");
            }
        } else {
            println!("跳过缓存测试，因为网络请求失败");
        }
    }
}
