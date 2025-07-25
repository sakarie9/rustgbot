use anyhow::{Result, anyhow};
use common::get_env_var;

use crate::auth::get_access_token_with_retry;
use crate::constants::PIXIV_UA;
use crate::models::{PixivApiResponse, PixivAppApiResponse, PixivPagesResponse};

/// 获取 Pixiv 作品信息（Ajax API）
pub async fn get_pixiv_info(id: &str) -> Result<PixivApiResponse> {
    log::debug!("Fetching Pixiv image with ID: {}", id);

    // 构建 Pixiv API URL
    let api_url = format!("https://www.pixiv.net/ajax/illust/{}", id);
    log::debug!("Pixiv API URL: {}", api_url);

    // 创建HTTP客户端，设置必要的请求头
    let client = reqwest::Client::new();
    let response = client
        .get(&api_url)
        .header("User-Agent", PIXIV_UA)
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
    log::trace!("Pixiv API response: {}", text);

    // 解析JSON响应
    let api_response: PixivApiResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("Failed to parse Pixiv API response: {}", e))?;

    if api_response.error {
        return Err(anyhow!("Pixiv API error: {}", api_response.message));
    }

    Ok(api_response)
}

/// 获取多页图片信息（Ajax API）
pub async fn get_pixiv_pages(id: &str) -> Result<PixivPagesResponse> {
    let client = reqwest::Client::new();
    let page_url = format!("https://www.pixiv.net/ajax/illust/{}/pages", id);

    let page_response = client
        .get(&page_url)
        .header("User-Agent", PIXIV_UA)
        .header("Referer", "https://www.pixiv.net/")
        .send()
        .await?;

    if !page_response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch Pixiv pages: HTTP {}",
            page_response.status()
        ));
    }

    let page_text = page_response.text().await?;
    let page_data: PixivPagesResponse = serde_json::from_str(&page_text)
        .map_err(|e| anyhow!("Failed to parse Pixiv pages response: {}", e))?;

    Ok(page_data)
}

/// 获取 R18 内容的图片 URL（使用 App API）
pub async fn get_r18_image_urls(id: &str) -> Result<Vec<String>> {
    if get_env_var("PIXIV_REFRESH_TOKEN").is_none() {
        return Err(anyhow!("No refresh token available for R18 content"));
    }

    let token = get_access_token_with_retry().await?;

    let client = reqwest::Client::new();
    let app_api_url = format!(
        "https://app-api.pixiv.net/v1/illust/detail?illust_id={}&filter=for_ios",
        id
    );

    let response = client
        .get(&app_api_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", PIXIV_UA)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch R18 image URLs: HTTP {}",
            response.status()
        ));
    }

    let text = response.text().await?;
    log::trace!("Pixiv App API response: {}", text);

    let app_response: PixivAppApiResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("Failed to parse Pixiv App API response: {}", e))?;

    let urls: Vec<String> = app_response
        .illust
        .meta_pages
        .into_iter()
        .map(|page| page.image_urls.original)
        .collect();

    Ok(urls)
}
