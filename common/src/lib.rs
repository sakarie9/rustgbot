//! 共用工具函数库
//!
//! 这个模块包含了整个workspace中可能用到的通用工具函数。
use anyhow::{Result, anyhow};
use human_bytes::human_bytes;
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
                log::debug!("File size: {} bytes ({})", size, convert_bytes(size as f64));

                if size > MAX_FILE_SIZE {
                    return Err(anyhow!(
                        "File too large: {} (max: {})",
                        convert_bytes(size as f64),
                        convert_bytes(MAX_FILE_SIZE as f64)
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

    let bytes_len = bytes.len();

    // 再次检查实际下载的文件大小
    if bytes_len > MAX_FILE_SIZE {
        return Err(anyhow!(
            "Downloaded file too large: {} (max: {})",
            convert_bytes(bytes_len as f64),
            convert_bytes(MAX_FILE_SIZE as f64)
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

/// 将字节数转换为人类可读的格式
pub fn convert_bytes<T: Into<f64>>(bytes: T) -> String {
    human_bytes(bytes.into())
}

/// 从URL中提取文件名，如果无法提取则根据content-type生成默认文件名
pub fn extract_filename_from_url(url: &str, content_type: &str) -> String {
    use std::path::Path;

    // 尝试从URL路径中提取文件名
    if let Ok(parsed_url) = url::Url::parse(url) {
        let path = parsed_url.path();
        if let Some(filename) = Path::new(path).file_name() {
            if let Some(filename_str) = filename.to_str() {
                if !filename_str.is_empty() && filename_str != "/" {
                    return filename_str.to_string();
                }
            }
        }
    }

    // 如果无法从URL提取文件名，根据content-type生成默认文件名
    get_file_extension_from_content_type(content_type)
}

/// 根据URL的文件扩展名推断Content-Type
pub fn guess_content_type_from_url(url: &str) -> Option<String> {
    use std::path::Path;

    // 尝试从URL中提取文件扩展名
    if let Ok(parsed_url) = url::Url::parse(url) {
        let path = parsed_url.path();
        if let Some(extension) = Path::new(path).extension() {
            if let Some(ext_str) = extension.to_str() {
                return Some(match ext_str.to_lowercase().as_str() {
                    // 图片格式
                    "jpg" | "jpeg" => "image/jpeg".to_string(),
                    "png" => "image/png".to_string(),
                    "gif" => "image/gif".to_string(),
                    "webp" => "image/webp".to_string(),
                    "bmp" => "image/bmp".to_string(),
                    "svg" => "image/svg+xml".to_string(),

                    // 视频格式
                    "mp4" => "video/mp4".to_string(),
                    "webm" => "video/webm".to_string(),
                    "avi" => "video/x-msvideo".to_string(),
                    "mov" => "video/quicktime".to_string(),
                    "mkv" => "video/x-matroska".to_string(),

                    // 音频格式
                    "mp3" => "audio/mpeg".to_string(),
                    "wav" => "audio/wav".to_string(),
                    "ogg" => "audio/ogg".to_string(),
                    "flac" => "audio/flac".to_string(),
                    "aac" => "audio/aac".to_string(),

                    // 文档格式
                    "pdf" => "application/pdf".to_string(),
                    "zip" => "application/zip".to_string(),
                    "rar" => "application/x-rar-compressed".to_string(),
                    "7z" => "application/x-7z-compressed".to_string(),
                    "txt" => "text/plain".to_string(),

                    // 其他格式保持为 application/octet-stream
                    _ => "application/octet-stream".to_string(),
                });
            }
        }
    }
    None
}

/// 根据content-type获取对应的文件扩展名
pub fn get_file_extension_from_content_type(content_type: &str) -> String {
    let extension = if content_type.starts_with("image/") {
        match content_type {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => "jpg", // 默认图片格式
        }
    } else if content_type.starts_with("video/") {
        match content_type {
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            "video/avi" => "avi",
            _ => "mp4", // 默认视频格式
        }
    } else if content_type.starts_with("audio/") {
        match content_type {
            "audio/mpeg" => "mp3",
            "audio/wav" => "wav",
            "audio/ogg" => "ogg",
            _ => "mp3", // 默认音频格式
        }
    } else {
        match content_type {
            "application/pdf" => "pdf",
            "application/zip" => "zip",
            "text/plain" => "txt",
            _ => "bin",
        }
    };

    format!("file.{}", extension)
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
