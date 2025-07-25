use anyhow::{Ok, Result, anyhow};
use common::{
    LinkProcessor, ProcessorError, ProcessorResult, ProcessorResultMedia, ProcessorResultType,
    join_url,
};
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;
use url::Url;

mod tests;

static PIXIV_REGEX: OnceLock<Regex> = OnceLock::new();

const FALLBACK_URL: &str = "https://pixiv.cat/";
const REVERSE_PROXY_URL: &str = "https://i.pixiv.cat/";

/// Pixiv链接处理器
pub struct PixivLinkProcessor;

impl PixivLinkProcessor {
    const PATTERN: &'static str = r"(?:https?://)?(?:www\.)?pixiv\.net/artworks/(\d+)(?:\?p=\d+)?";
}

#[async_trait::async_trait]
impl LinkProcessor for PixivLinkProcessor {
    fn pattern(&self) -> &'static str {
        Self::PATTERN
    }

    fn regex(&self) -> &Regex {
        PIXIV_REGEX.get_or_init(|| Regex::new(Self::PATTERN).expect("Invalid Pixiv regex pattern"))
    }

    async fn process_captures(&self, captures: &regex::Captures<'_>) -> ProcessorResultType {
        if let Some(id_match) = captures.get(1) {
            let id = id_match.as_str();
            match get_pixiv(id).await {
                std::result::Result::Ok(parsed) => {
                    std::result::Result::Ok(ProcessorResult::Media(parsed))
                }
                std::result::Result::Err(e) => std::result::Result::Err(
                    ProcessorError::with_source("处理Pixiv链接失败", e.to_string()),
                ),
            }
        } else {
            std::result::Result::Err(ProcessorError::new("无法从Pixiv链接中提取作品ID"))
        }
    }

    fn name(&self) -> &'static str {
        "Pixiv"
    }
}

/// 获取Pixiv图片，支持代理URL转换
async fn get_pixiv(id: &str) -> Result<ProcessorResultMedia> {
    let mut result = get_pixiv_image(id).await?;

    let use_proxy = true;

    if use_proxy {
        // 将Pixiv图片URL转换为代理URL
        result.urls = result
            .urls
            .into_iter()
            .map(|url| convert_to_proxy_url(&url))
            .collect::<Result<Vec<_>, _>>()?;
    }

    Ok(result)
}

/// 将Pixiv原始URL转换为代理URL
fn convert_to_proxy_url(original_url: &str) -> Result<String> {
    let reverse_url = Url::parse(REVERSE_PROXY_URL)?;
    // 使用常见的Pixiv代理服务
    // 例如：i.pixiv.re, i.pixiv.cat, i.pximg.net等
    let result = if original_url.contains("i.pximg.net") {
        original_url.replace(
            "i.pximg.net",
            reverse_url
                .domain()
                .ok_or(anyhow!("Invalid reverse proxy URL"))?,
        )
    } else {
        // 如果URL格式不符合预期，返回原URL
        original_url.to_string()
    };

    Ok(result)
}

#[derive(Debug, Deserialize)]
struct PixivApiResponse {
    error: bool,
    message: String,
    body: Option<PixivIllustBody>,
}

#[derive(Debug, Deserialize)]
struct PixivIllustBody {
    title: String,
    #[serde(rename = "userName")]
    user_name: String,
    description: String,
    #[serde(rename = "pageCount")]
    page_count: u32,
    urls: PixivUrls,
}

#[derive(Debug, Deserialize)]
struct PixivUrls {
    original: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PixivPagesResponse {
    error: bool,
    body: Option<Vec<PixivPageInfo>>,
}

#[derive(Debug, Deserialize)]
struct PixivPageInfo {
    urls: PixivUrls,
}

async fn get_pixiv_image(id: &str) -> Result<ProcessorResultMedia> {
    log::debug!("Fetching Pixiv image with ID: {}", id);

    // 构建 Pixiv API URL
    let api_url = format!("https://www.pixiv.net/ajax/illust/{}", id);
    log::debug!("Pixiv API URL: {}", api_url);

    // 创建HTTP客户端，设置必要的请求头
    let client = reqwest::Client::new();
    let response = client
        .get(&api_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
        .header("Referer", "https://www.pixiv.net/")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch Pixiv data: HTTP {}",
            response.status()
        ));
    }

    let text = response.text().await?;
    log::debug!("Pixiv API response: {}", text);

    // 解析JSON响应
    let api_response: PixivApiResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("Failed to parse Pixiv API response: {}", e))?;

    if api_response.error {
        return Err(anyhow!("Pixiv API error: {}", api_response.message));
    }

    let body = api_response
        .body
        .ok_or_else(|| anyhow!("Empty response body from Pixiv API"))?;

    // 构建返回文本
    let description_text = if body.description.is_empty() {
        "无描述".to_string()
    } else {
        // 移除HTML标签
        let re = regex::Regex::new(r"<[^>]*>")?;
        re.replace_all(&body.description, "").to_string()
    };

    let text = format!(
        "🎨 Pixiv 作品\n\n📋 标题: {}\n👤 作者: {}\n📄 描述: {}",
        body.title, body.user_name, description_text
    );

    // 处理图片URL
    let mut urls = Vec::new();

    if body.page_count == 1 {
        // 单张图片
        if let Some(original_url) = &body.urls.original {
            urls.push(original_url.clone());
        } else {
            // R18内容，使用pixiv.cat镜像
            let fallback_url = join_url(FALLBACK_URL, &format!("{}.jpg", id))?;
            urls.push(fallback_url);
            log::debug!("Using pixiv.cat fallback URL for R18 content: {}", id);
        }
    } else {
        // 多张图片，需要获取每一页
        let page_url = format!("https://www.pixiv.net/ajax/illust/{}/pages", id);
        let page_response = client
            .get(&page_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .header("Referer", "https://www.pixiv.net/")
            .send()
            .await?;

        let mut pages_fetched = false;
        if page_response.status().is_success() {
            let page_text = page_response.text().await?;
            let page_data: PixivPagesResponse = serde_json::from_str(&page_text)
                .map_err(|e| anyhow!("Failed to parse Pixiv pages response: {}", e))?;

            if !page_data.error {
                if let Some(pages) = page_data.body {
                    for (index, page_info) in pages.iter().enumerate() {
                        if let Some(original_url) = &page_info.urls.original {
                            urls.push(original_url.clone());
                        } else {
                            // R18内容，使用pixiv.cat镜像 (多张图片格式)
                            let fallback_url =
                                join_url(FALLBACK_URL, &format!("{}-{}.jpg", id, index + 1))?;
                            urls.push(fallback_url);
                        }
                    }
                    pages_fetched = true;
                }
            }
        }

        // 如果获取多页失败，根据page_count生成pixiv.cat链接
        if !pages_fetched {
            if let Some(original_url) = &body.urls.original {
                urls.push(original_url.clone());
            } else {
                // 为多张图片生成pixiv.cat链接
                for i in 1..=body.page_count {
                    let fallback_url = join_url(FALLBACK_URL, &format!("{}-{}.jpg", id, i))?;
                    urls.push(fallback_url);
                }
                log::debug!(
                    "Using pixiv.cat fallback URLs for R18 multi-page content: {} pages",
                    body.page_count
                );
            }
        }
    }

    Ok(ProcessorResultMedia {
        caption: text,
        urls,
    })
}
