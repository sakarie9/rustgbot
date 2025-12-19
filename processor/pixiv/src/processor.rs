use anyhow::Result;
use common::ProcessorResultMedia;

use crate::api::get_pixiv_info;
use crate::utils::{build_pixiv_caption, convert_to_proxy_url, get_urls_from_count};

/// 获取Pixiv图片，支持代理URL转换
pub async fn get_pixiv(id: &str) -> Result<ProcessorResultMedia> {
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

async fn get_pixiv_image(id: &str) -> Result<ProcessorResultMedia> {
    let api_response = get_pixiv_info(id).await?;

    let body = api_response
        .body
        .ok_or_else(|| anyhow::anyhow!("Empty response body from Pixiv API"))?;

    // 构建返回文本
    let text = build_pixiv_caption(&body)?;

    // 处理图片URL
    // HACK: Use regular quality instead of original to avoid telegram limit
    let Some(url) = body.urls.regular.as_ref() else {
        // 空图片URL，返回文本结果
        log::error!("No regular image URL found for Pixiv ID: {}", id);
        return Ok(ProcessorResultMedia {
            caption: text,
            urls: Vec::new(),
            spoiler: false,
            original_urls: None,
        });
    };

    let image_urls = if body.page_count > 1 {
        get_urls_from_count(url, body.page_count)
    } else {
        vec![url.to_string()]
    };

    // 检查 x_restrict 值
    let is_restrict = body.x_restrict > 0;

    Ok(ProcessorResultMedia {
        caption: text,
        urls: image_urls.clone(),        // 这里会在后续被代理URL替换
        spoiler: is_restrict,               // 如果是限制内容，设置 spoiler 为 true
        original_urls: Some(image_urls), // 保存原始URL用于下载
    })
}
