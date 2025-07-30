//! 共用工具函数库
//!
//! 这个模块包含了整个workspace中可能用到的通用工具函数。
use anyhow::{Result, anyhow};
use url::Url;
pub mod models;
pub use models::*;

pub const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10MB
pub const GENERAL_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36";
pub const SUMMARY_MAX_LENGTH: usize = 600;
pub const SUMMARY_MAX_MAX_LENGTH: usize = 800;

/// 获取环境变量的值
pub fn get_env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// 使用url库安全地拼接URL，避免斜杠重复
pub fn join_url(base: &str, path: &str) -> Result<String> {
    let base_url = Url::parse(base)?;
    let joined = base_url.join(path)?;
    Ok(joined.to_string())
}

// 下载任意文件的通用函数
pub async fn download_file(url: &str) -> Result<(Vec<u8>, String)> {
    download_file_ua(url, GENERAL_UA).await
}

pub async fn download_file_ua(url: &str, ua: &str) -> Result<(Vec<u8>, String)> {
    download_file_internal(url, ua, None).await
}

// 下载 GIF 文件的辅助函数
pub async fn get_gif_bytes(url: &str) -> Result<Vec<u8>> {
    get_gif_bytes_ua(url, GENERAL_UA).await
}

pub async fn get_gif_bytes_ua(url: &str, ua: &str) -> Result<Vec<u8>> {
    let (bytes, _) = download_file_internal(url, ua, Some("gif".to_string())).await?;
    Ok(bytes)
}

// 内部下载函数，统一处理所有下载逻辑
async fn download_file_internal(
    url: &str,
    ua: &str,
    check_image_type: Option<String>,
) -> Result<(Vec<u8>, String)> {
    let client = reqwest::Client::builder().user_agent(ua).build()?;

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
                log::debug!(
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

    // 获取内容类型
    let content_type = head_response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    log::debug!("Content-Type: {}", content_type);

    // 如果需要检查类型
    if let Some(ref check_type) = check_image_type {
        if !content_type.contains(check_type) {
            return Err(anyhow!(
                "Content-Type {} does not match expected type {}",
                content_type,
                check_type
            ));
        }
    }

    // 如果检查通过，开始实际下载
    log::debug!("Starting download from: {}", url);
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!("HTTP GET request failed: {}", response.status()));
    }

    let bytes = response.bytes().await?;

    // 再次检查实际下载的文件大小
    if bytes.len() > MAX_FILE_SIZE {
        return Err(anyhow!(
            "Downloaded file too large: {:.2} MB (max: {:.2} MB)",
            bytes.len() as f64 / 1024.0 / 1024.0,
            MAX_FILE_SIZE as f64 / 1024.0 / 1024.0
        ));
    }

    log::debug!("Successfully downloaded {} bytes", bytes.len());
    Ok((bytes.to_vec(), content_type))
}

/// 截断描述文本到指定长度
pub fn substring_desc(desc: &str) -> String {
    let chars: Vec<char> = desc.chars().collect();

    // 如果字符数没有超过最大长度，直接返回
    if chars.len() <= SUMMARY_MAX_LENGTH {
        return desc.trim().to_string();
    }

    // 在最大长度位置之后查找换行符
    let mut cr_pos = None;

    // 从 SUMMARY_MAX_LENGTH 位置开始查找换行符
    for i in SUMMARY_MAX_LENGTH..chars.len() {
        if chars[i] == '\n' {
            cr_pos = Some(i);
            break;
        }
    }

    match cr_pos {
        Some(pos) if pos < SUMMARY_MAX_MAX_LENGTH => {
            // 换行符在最大长度和极限长度之间，裁剪到换行符
            chars[..pos].iter().collect::<String>().trim().to_string()
        }
        _ => {
            // 没有找到合适的换行符，或换行符超过极限长度，直接截取到最大长度并添加省略号
            let truncated: String = chars[..SUMMARY_MAX_LENGTH].iter().collect();
            format!("{}……", truncated.trim())
        }
    }
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

    #[test]
    fn test_url_joining() {
        let test_cases = vec![
            (
                "https://pixiv.cat/",
                "114514.jpg",
                "https://pixiv.cat/114514.jpg",
            ),
            (
                "https://pixiv.cat",
                "114514.jpg",
                "https://pixiv.cat/114514.jpg",
            ),
            (
                "https://pixiv.cat/",
                "/114514.jpg",
                "https://pixiv.cat/114514.jpg",
            ),
            (
                "https://pixiv.cat",
                "/114514.jpg",
                "https://pixiv.cat/114514.jpg",
            ),
        ];

        for (base, path, expected) in test_cases {
            let result = join_url(base, path).unwrap();
            assert_eq!(result, expected);
            println!("✓ Base: {} + Path: {} = {}", base, path, result);
        }
    }
}
