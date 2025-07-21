//! 共用工具函数库
//!
//! 这个模块包含了整个workspace中可能用到的通用工具函数。
use anyhow::{Result, anyhow};

pub const NGA_UA: &str = "NGA_skull/6.0.5(iPhone10,3;iOS 12.0.1)";

pub const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10MB

/// 获取环境变量的值
pub fn get_env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

// 下载 GIF 文件的辅助函数
pub async fn get_gif_bytes(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder().user_agent(NGA_UA).build()?;

    // 先发送 HEAD 请求检查文件大小和类型
    let head_response = client.head(url).send().await?;

    if !head_response.status().is_success() {
        return Err(anyhow!(
            "HTTP HEAD request failed: {}",
            head_response.status()
        ));
    }

    // 检查内容长度
    if let Some(content_length) = head_response.headers().get("content-length") {
        if let Ok(size_str) = content_length.to_str() {
            if let Ok(size) = size_str.parse::<usize>() {
                log::info!(
                    "File size: {} bytes ({:.2} MB)",
                    size,
                    size as f64 / 1024.0 / 1024.0
                );

                if size > MAX_FILE_SIZE {
                    return Err(anyhow!(
                        "File too large: {:.2} MB (max: {:.2} MB)",
                        size as f64 / 1024.0 / 1024.0,
                        MAX_FILE_SIZE as f64 / 1024.0 / 1024.0
                    ));
                }
            }
        }
    } else {
        log::debug!("Content-Length header not found, proceeding with download");
    }

    // 检查内容类型
    if let Some(content_type) = head_response.headers().get("content-type") {
        let content_type_str = content_type.to_str().unwrap_or("");
        log::debug!("Content-Type: {}", content_type_str);

        // 如果不是图片类型，记录警告但继续处理
        if !content_type_str.starts_with("image/") {
            log::debug!(
                "Warning: Content-Type is not image, but proceeding anyway: {}",
                content_type_str
            );
        }
    }

    // 如果检查通过，开始实际下载
    log::debug!("Starting download from: {}", url);
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!("HTTP GET request failed: {}", response.status()));
    }

    let bytes = response.bytes().await?;

    // 再次检查实际下载的文件大小（防止服务器返回的 Content-Length 不准确）
    if bytes.len() > MAX_FILE_SIZE {
        return Err(anyhow!(
            "Downloaded file too large: {:.2} MB (max: {:.2} MB)",
            bytes.len() as f64 / 1024.0 / 1024.0,
            MAX_FILE_SIZE as f64 / 1024.0 / 1024.0
        ));
    }

    log::debug!("Successfully downloaded {} bytes", bytes.len());
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_env_var() {
        // 测试获取一个存在的环境变量
        unsafe {
            std::env::set_var("TEST_VAR", "test_value");
        }
        let value = get_env_var("TEST_VAR");
        assert_eq!(value, Some("test_value".to_string()));

        // 测试获取一个不存在的环境变量
        let missing_value = get_env_var("MISSING_VAR");
        assert_eq!(missing_value, None);
    }
}
