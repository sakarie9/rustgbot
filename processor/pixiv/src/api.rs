use anyhow::{Result, anyhow};
use common::get_env_var;

use crate::constants::PIXIV_UA;
use crate::models::{PixivApiResponse};

/// 获取 Pixiv 作品信息（Ajax API）
pub async fn get_pixiv_info(id: &str) -> Result<PixivApiResponse> {
    log::debug!("Fetching Pixiv image with ID: {}", id);

    // 构建 Pixiv API URL
    let api_url = format!("https://www.pixiv.net/ajax/illust/{}", id);
    log::debug!("Pixiv API URL: {}", api_url);

    // 创建HTTP客户端，设置必要的请求头
    let client = reqwest::Client::new();
    let request = client
        .get(&api_url)
        .header("User-Agent", PIXIV_UA)
        .header("Referer", "https://www.pixiv.net/");

    // 如果有PHPSESSID环境变量，添加到请求头
    let request = if let Some(session_id) = get_env_var("PIXIV_COOKIE") {
        request.header("Cookie", format!("PHPSESSID={}", session_id))
    } else {
        request
    };

    let response = request.send()
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
