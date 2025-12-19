use anyhow::{Result, anyhow};
use common::{get_env_var, join_url};
use url::Url;

use crate::constants::REVERSE_PROXY_URL;
use crate::models::PixivIllustBody;

/// 获取反向代理URL
fn get_reverse_proxy_url() -> Result<String> {
    let url = get_env_var("PIXIV_IMAGE_PROXY").unwrap_or_else(|| {
        // 如果环境变量未设置，使用默认值
        REVERSE_PROXY_URL.to_string()
    });

    // 验证URL格式
    Url::parse(&url)
        .map_err(|e| anyhow!("Invalid reverse proxy URL: {}", e))
        .map(|url| url.to_string())
}

/// 将Pixiv原始URL转换为代理URL
pub fn convert_to_proxy_url(original_url: &str) -> Result<String> {
    let original_url = Url::parse(original_url)?;
    let proxy_url = Url::parse(get_reverse_proxy_url()?.as_str())?;

    let relative_path = original_url
        .path()
        .strip_prefix("/")
        .unwrap_or(original_url.path());

    let mut final_url = proxy_url.join(relative_path)?;

    // 将原始 URL 的查询参数（?后面的部分）附加到新 URL 上
    if let Some(query) = original_url.query() {
        final_url.set_query(Some(query));
    }

    Ok(final_url.to_string())
}

/// 构建Pixiv作品的标题文本
pub fn build_pixiv_caption(body: &PixivIllustBody) -> Result<String> {
    // 构建描述文本，清理HTML标签
    let description_text = if body.description.is_empty() {
        None
    } else {
        let cleaned_desc = body
            .description
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("<br />", "\n");
        Some(cleaned_desc)
    };

    // 处理tags
    let tags_text = if let Some(tags_data) = &body.tags {
        let tag_names: Vec<String> = tags_data
            .tags
            .iter()
            .map(|t| {
                let processed_tag = if t.tag == "R-18" {
                    "R18".to_string()
                } else {
                    t.tag.clone()
                };
                format!("#{}", processed_tag)
            })
            .collect();
        if !tag_names.is_empty() {
            Some(tag_names.join(", "))
        } else {
            None
        }
    } else {
        None
    };

    // 构建文本，只显示非空字段
    let mut text = format!(
        "<b><u><a href=\"{}\">{}</a></u></b> / <b><u><a href=\"{}\">{}</a></u></b>",
        join_url("https://www.pixiv.net/artworks/", &body.id)?,
        body.title,
        join_url("https://www.pixiv.net/users/", &body.user_id)?,
        body.user_name
    );

    if let Some(desc) = &description_text {
        // 截取
        let truncated_desc = common::substring_desc(desc);
        text.push_str(&format!("\n\n{}", truncated_desc));
    }

    if let Some(tags) = tags_text {
        text.push_str(&format!("\n\n{}", tags));
    }

    Ok(text)
}

// Build real image URLs directly from the first page URL and total page count
// https://i.pixiv.net/img-original/img/2024/11/30/00/00/47/124748386_p0.png
// https://i.pixiv.net/img-original/img/2024/11/30/00/00/47/124748386_p1.png
pub fn get_urls_from_count(url: &str, count: u32) -> Vec<String> {
    if !url.contains("_p0") {
        return vec![url.to_string()];
    }
    let mut urls = Vec::new();
    for i in 0..count {
        let page_url = url.replace("_p0", &format!("_p{}", i));
        urls.push(page_url);
    }
    urls
}
