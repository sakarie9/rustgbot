use anyhow::Result;
use common::{ProcessorResultMedia, get_env_var};

use crate::api::{get_pixiv_info, get_pixiv_pages, get_r18_image_urls};
use crate::utils::{build_pixiv_caption, convert_to_proxy_url};

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
    let mut urls = Vec::new();

    // 检查 x_restrict 值
    let is_restrict = body.x_restrict == 1;
    if !is_restrict {
        // 普通内容，直接使用 Ajax API 获取的 URL
        log::debug!("Normal content detected for ID: {}", id);

        if body.page_count == 1 {
            // 单张图片
            if let Some(original_url) = &body.urls.original {
                urls.push(original_url.clone());
            }
        } else {
            // 多张图片，需要获取每一页
            match get_pixiv_pages(id).await {
                Ok(page_data) => {
                    if !page_data.error {
                        if let Some(pages) = page_data.body {
                            for page_info in pages.iter() {
                                if let Some(original_url) = &page_info.urls.original {
                                    urls.push(original_url.clone());
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    // 如果获取多页失败，使用第一张图片的URL
                    if let Some(original_url) = &body.urls.original {
                        urls.push(original_url.clone());
                    }
                }
            }
        }
    } else if is_restrict {
        // R18 内容
        log::debug!("R18 content detected for ID: {}", id);

        if get_env_var("PIXIV_REFRESH_TOKEN").is_some() {
            // 有 refresh token，使用 App API 获取原始图片 URL
            match get_r18_image_urls(id).await {
                Ok(r18_urls) => {
                    if !r18_urls.is_empty() {
                        urls = r18_urls;
                        log::debug!(
                            "Successfully fetched {} R18 image URLs via App API",
                            urls.len()
                        );
                    } else {
                        log::warn!("App API returned empty URLs for R18 content, ID: {}", id);
                    }
                }
                Err(e) => {
                    log::warn!("Failed to fetch R18 URLs via App API: {}, ID: {}", e, id);
                }
            }
        } else {
            // 没有 refresh token，不返回 URL
            log::debug!(
                "No refresh token available for R18 content, returning empty URLs for ID: {}",
                id
            );
        }
    }

    Ok(ProcessorResultMedia {
        caption: text,
        urls,
        spoiler: is_restrict, // 如果是限制内容，设置 spoiler 为 true
    })
}
