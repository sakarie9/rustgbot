//! 共用工具函数库
//!
//! 这个模块包含了整个workspace中可能用到的通用工具函数。
use anyhow::{Result, anyhow};
use byte_unit::Byte;
use human_bytes::human_bytes;
use std::cell::RefCell;
use url::Url;

pub mod models;
pub use models::*;

const DEFAULT_MAX_FILE_SIZE: usize = 10 * 1000 * 1000; // 默认最大文件大小：10MB
pub const GENERAL_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36";
pub const PIXIV_REFERER: &str = "https://www.pixiv.net/";
pub const SUMMARY_MAX_LENGTH: usize = 600;
pub const SUMMARY_MAX_MAX_LENGTH: usize = 800;

/// 获取最大文件大小设置，支持从环境变量 MAX_FILE_SIZE 读取
/// 环境变量值可以是字节数（如 "10485760"）或人类可读格式（如 "10MB", "1GB"）
/// 如果无法解析则使用默认值 10MB
///
/// https://core.telegram.org/bots/api#sendphoto
/// The photo must be at most 10 MB in size.
pub fn get_max_file_size() -> usize {
    match get_env_var("MAX_FILE_SIZE") {
        Some(size_str) => {
            // 先尝试直接解析为数字（字节数）
            if let Ok(size) = size_str.parse::<usize>() {
                log::debug!(
                    "Using MAX_FILE_SIZE from environment: {} bytes ({})",
                    size,
                    convert_bytes(size as f64)
                );
                return size;
            }

            // 如果不是纯数字，尝试解析人类可读格式
            match Byte::parse_str(&size_str, true) {
                Ok(byte_obj) => {
                    let size = byte_obj.as_u64() as usize;
                    log::debug!(
                        "Using MAX_FILE_SIZE from environment: {} -> {} bytes ({})",
                        size_str,
                        size,
                        convert_bytes(size as f64)
                    );
                    size
                }
                Err(_) => {
                    log::warn!(
                        "Invalid MAX_FILE_SIZE environment variable: {}, using default: {} bytes",
                        size_str,
                        DEFAULT_MAX_FILE_SIZE
                    );
                    DEFAULT_MAX_FILE_SIZE
                }
            }
        }
        None => {
            log::debug!(
                "MAX_FILE_SIZE not set, using default: {} bytes ({})",
                DEFAULT_MAX_FILE_SIZE,
                convert_bytes(DEFAULT_MAX_FILE_SIZE as f64)
            );
            DEFAULT_MAX_FILE_SIZE
        }
    }
}

// 线程局部存储，控制是否启用文本截断
thread_local! {
    static TRUNCATION_ENABLED: RefCell<bool> = const { RefCell::new(true) };
}

/// 设置是否启用文本截断
pub fn set_truncation_enabled(enabled: bool) {
    TRUNCATION_ENABLED.with(|flag| {
        *flag.borrow_mut() = enabled;
    });
}

/// 获取当前是否启用文本截断
pub fn is_truncation_enabled() -> bool {
    TRUNCATION_ENABLED.with(|flag| *flag.borrow())
}

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
    download_file_internal(url, ua, None, None).await
}

pub async fn download_pixiv(url: &str) -> Result<(Vec<u8>, String)> {
    download_file_internal(url, GENERAL_UA, Some(PIXIV_REFERER), None).await
}

// 下载 GIF 文件的辅助函数
pub async fn get_gif_bytes(url: &str) -> Result<Vec<u8>> {
    get_gif_bytes_ua(url, GENERAL_UA).await
}

pub async fn get_gif_bytes_ua(url: &str, ua: &str) -> Result<Vec<u8>> {
    let (bytes, _) = download_file_internal(url, ua, None, Some("gif".to_string())).await?;
    Ok(bytes)
}

// 内部下载函数，统一处理所有下载逻辑
async fn download_file_internal(
    url: &str,
    ua: &str,
    referer: Option<&str>,
    check_image_type: Option<String>,
) -> Result<(Vec<u8>, String)> {
    let client = reqwest::Client::builder().user_agent(ua).build()?;

    // 先发送 HEAD 请求检查文件大小和类型
    let mut head_response = client.head(url);

    if let Some(referer) = referer {
        head_response = head_response.header("Referer", referer);
    }

    let head_response = head_response.send().await?;

    if !head_response.status().is_success() {
        return Err(anyhow!(
            "HTTP HEAD request failed: {}",
            head_response.status()
        ));
    }

    // 检查内容长度
    if let Some(content_length) = head_response.headers().get("content-length") {
        if let Ok(size_str) = content_length.to_str()
            && let Ok(size) = size_str.parse::<usize>()
        {
            log::debug!("File size: {} bytes ({})", size, convert_bytes(size as f64));

            let max_file_size = get_max_file_size();
            if size > max_file_size {
                return Err(anyhow!(
                    "File too large: {} (max: {})",
                    convert_bytes(size as f64),
                    convert_bytes(max_file_size as f64)
                ));
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
    if let Some(ref check_type) = check_image_type
        && !content_type.contains(check_type)
    {
        return Err(anyhow!(
            "Content-Type {} does not match expected type {}",
            content_type,
            check_type
        ));
    }

    // 如果检查通过，开始实际下载
    log::debug!("Starting download from: {}", url);
    let mut response = client.get(url);

    if let Some(referer) = referer {
        response = response.header("Referer", referer);
    }

    let response = response.send().await?;

    if !response.status().is_success() {
        return Err(anyhow!("HTTP GET request failed: {}", response.status()));
    }

    let bytes = response.bytes().await?;

    let bytes_len = bytes.len();

    // 再次检查实际下载的文件大小
    let max_file_size = get_max_file_size();
    if bytes_len > max_file_size {
        return Err(anyhow!(
            "Downloaded file too large: {} (max: {})",
            convert_bytes(bytes_len as f64),
            convert_bytes(max_file_size as f64)
        ));
    }

    log::info!(
        "Successfully downloaded {}",
        convert_bytes(bytes_len as f64)
    );
    Ok((bytes.to_vec(), content_type))
}

/// 截断描述文本到指定长度
pub fn substring_desc(desc: &str) -> String {
    // 检查是否启用截断
    if !is_truncation_enabled() {
        return desc.trim().to_string();
    }

    substring_desc_with_truncation(desc, true)
}

/// 控制是否截断描述文本
pub fn substring_desc_with_truncation(desc: &str, should_truncate: bool) -> String {
    if !should_truncate {
        return desc.trim().to_string();
    }

    let chars: Vec<char> = desc.chars().collect();

    // 如果字符数没有超过最大长度，直接返回
    if chars.len() <= SUMMARY_MAX_LENGTH {
        return desc.trim().to_string();
    }

    // 在最大长度位置之后查找换行符
    let mut cr_pos = None;

    // 从 SUMMARY_MAX_LENGTH 位置开始查找换行符
    for (i, c) in chars.iter().enumerate().skip(SUMMARY_MAX_LENGTH) {
        if *c == '\n' {
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

/// 从URL中提取文件名，优先使用content-type推断的文件扩展名，无法确定时使用URL的扩展名
pub fn extract_filename_from_url(url: &str, content_type: &str) -> String {
    use std::path::Path;

    // 获取从content-type推断的扩展名
    let content_type_extension = get_file_extension_from_content_type(content_type);

    // 尝试从URL路径中提取文件名
    if let Ok(parsed_url) = url::Url::parse(url) {
        let path = parsed_url.path();
        if let Some(filename) = Path::new(path).file_name()
            && let Some(filename_str) = filename.to_str()
            && !filename_str.is_empty()
            && filename_str != "/"
        {
            let path_obj = Path::new(filename_str);

            // 获取文件名主体部分（不带扩展名）
            let stem = path_obj
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file");

            // 优先使用content-type的扩展名，如果没有则使用URL的扩展名
            let final_extension = content_type_extension.or_else(|| {
                path_obj
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_string())
            });

            match final_extension {
                Some(ext) => format!("{}.{}", stem, ext),
                None => stem.to_string(),
            }
        } else {
            // URL无法解析文件名的情况
            match content_type_extension {
                Some(ext) => format!("file.{}", ext),
                None => "file".to_string(),
            }
        }
    } else {
        // URL解析失败的情况
        match content_type_extension {
            Some(ext) => format!("file.{}", ext),
            None => "file".to_string(),
        }
    }
}

/// 根据URL的文件扩展名推断Content-Type
pub fn guess_content_type_from_url(url: &str) -> Option<String> {
    use std::path::Path;

    // 尝试从URL中提取文件扩展名
    if let Ok(parsed_url) = url::Url::parse(url) {
        let path = parsed_url.path();
        if let Some(extension) = Path::new(path).extension()
            && let Some(ext_str) = extension.to_str()
        {
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
    None
}

/// 根据content-type获取对应的文件扩展名，无法确定时返回None
pub fn get_file_extension_from_content_type(content_type: &str) -> Option<String> {
    let extension = if content_type.starts_with("image/") {
        match content_type {
            "image/jpeg" => Some("jpg"),
            "image/png" => Some("png"),
            "image/gif" => Some("gif"),
            "image/webp" => Some("webp"),
            _ => None, // 未知图片格式返回None
        }
    } else if content_type.starts_with("video/") {
        match content_type {
            "video/mp4" => Some("mp4"),
            "video/webm" => Some("webm"),
            "video/avi" => Some("avi"),
            _ => None, // 未知视频格式返回None
        }
    } else if content_type.starts_with("audio/") {
        match content_type {
            "audio/mpeg" => Some("mp3"),
            "audio/wav" => Some("wav"),
            "audio/ogg" => Some("ogg"),
            _ => None, // 未知音频格式返回None
        }
    } else {
        match content_type {
            "application/pdf" => Some("pdf"),
            "application/zip" => Some("zip"),
            "text/plain" => Some("txt"),
            _ => None, // 未知格式返回None
        }
    };

    extension.map(|ext| ext.to_string())
}

/// 验证图片尺寸是否符合Telegram的要求
///
/// Telegram对图片的要求：
/// - 宽度和高度的总和不能超过10000
/// - 宽高比不能超过20:1
/// - 宽度和高度都必须大于0
///
/// 返回: Ok(()) 如果图片尺寸有效，Err(String) 如果无效
pub fn validate_image_dimensions(image_data: &[u8]) -> Result<()> {
    match imagesize::blob_size(image_data) {
        Ok(size) => {
            let width = size.width;
            let height = size.height;

            log::debug!("Image dimensions: {}x{}", width, height);

            // 检查尺寸是否为0
            if width == 0 || height == 0 {
                return Err(anyhow!("Invalid image dimensions: {}x{}", width, height));
            }

            // 检查宽度+高度的总和
            if width + height > 10000 {
                return Err(anyhow!(
                    "Image dimensions too large: {}x{} (sum {} exceeds 10000)",
                    width,
                    height,
                    width + height
                ));
            }

            // 检查宽高比
            let ratio = if width > height {
                width as f64 / height as f64
            } else {
                height as f64 / width as f64
            };

            if ratio > 20.0 {
                return Err(anyhow!(
                    "Image aspect ratio too extreme: {}x{} (ratio {:.2} exceeds 20)",
                    width,
                    height,
                    ratio
                ));
            }

            Ok(())
        }
        Err(e) => Err(anyhow!("Failed to read image dimensions: {}", e)),
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

    #[test]
    fn test_extract_filename_from_url() {
        let test_cases = vec![
            // 正常情况，提取文件名并使用正确的扩展名
            ("https://example.com/image.png", "image/jpeg", "image.jpg"),
            ("https://example.com/video.avi", "video/mp4", "video.mp4"),
            (
                "https://example.com/document.txt",
                "application/pdf",
                "document.pdf",
            ),
            // URL没有扩展名的情况
            ("https://example.com/image", "image/png", "image.png"),
            // URL无法解析文件名的情况
            ("https://example.com/", "image/gif", "file.gif"),
            ("https://example.com", "video/mp4", "file.mp4"),
            // 未知content-type的情况，应该使用URL的扩展名
            (
                "https://example.com/document.xyz",
                "application/unknown",
                "document.xyz",
            ),
            (
                "https://example.com/image.png",
                "image/unknown",
                "image.png",
            ),
            ("https://example.com/", "application/unknown", "file"),
        ];

        for (url, content_type, expected) in test_cases {
            let result = extract_filename_from_url(url, content_type);
            assert_eq!(
                result, expected,
                "Failed for URL: {} with content-type: {}",
                url, content_type
            );
        }
    }

    #[test]
    fn test_get_file_extension_from_content_type() {
        let test_cases = vec![
            // 已知格式
            ("image/jpeg", Some("jpg")),
            ("image/png", Some("png")),
            ("video/mp4", Some("mp4")),
            ("audio/mpeg", Some("mp3")),
            ("application/pdf", Some("pdf")),
            // 未知格式应该返回None
            ("image/unknown", None),
            ("video/unknown", None),
            ("audio/unknown", None),
            ("application/unknown", None),
            ("text/unknown", None),
        ];

        for (content_type, expected) in test_cases {
            let result = get_file_extension_from_content_type(content_type);
            assert_eq!(
                result,
                expected.map(|s| s.to_string()),
                "Failed for content-type: {}",
                content_type
            );
        }
    }

    #[test]
    fn test_validate_image_dimensions() {
        // 创建一个简单的1x1 PNG图片数据 (最小的有效PNG)
        let valid_png: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // width=1, height=1
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE,
        ];

        // 测试有效图片
        assert!(validate_image_dimensions(&valid_png).is_ok());

        // 测试无效数据
        let invalid_data = vec![0x00, 0x01, 0x02];
        assert!(validate_image_dimensions(&invalid_data).is_err());
    }
}
