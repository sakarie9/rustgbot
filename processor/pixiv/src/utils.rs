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
    let reverse_url = Url::parse(get_reverse_proxy_url()?.as_str())?;
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
