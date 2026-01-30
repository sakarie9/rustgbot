//! NGA 页面抓取器

use crate::error::{NGAError, NGAResult};
use crate::page::NGAPage;
use crate::utils::{NGA_UA, get_nga_cookie, preprocess_url};

/// NGA 页面抓取器
pub struct NGAFetcher;

impl NGAFetcher {
    /// 解析 NGA 链接并返回处理结果
    pub async fn parse(url: &str) -> NGAResult<common::ProcessorResultMedia> {
        let processed_url = preprocess_url(url);
        let page = Self::fetch_page(&processed_url).await?;

        Ok(common::ProcessorResultMedia {
            caption: page.to_summary(),
            urls: page.images,
            spoiler: false,
            original_urls: None,
        })
    }

    /// 获取并解析 NGA 页面
    pub async fn fetch_page(url: &str) -> NGAResult<NGAPage> {
        let html = Self::fetch_html(url).await?;
        NGAPage::from_html(url, &html)
            .ok_or_else(|| NGAError::Parse("无法解析页面内容".to_string()))
    }

    /// 获取页面 HTML
    pub async fn fetch_html(url: &str) -> NGAResult<String> {
        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .header("User-Agent", NGA_UA)
            .header("Cookie", get_nga_cookie())
            .send()
            .await?;

        let status = response.status();

        if status.is_success() {
            response.text_with_charset("gbk").await.map_err(Into::into)
        } else {
            let status_code = status.as_u16();
            let message = match status_code {
                403 => "此帖子被锁定或无访问权限".to_string(),
                _ => format!("HTTP 请求失败，状态码: {}", status_code),
            };
            Err(NGAError::Http {
                status: status_code,
                message,
            })
        }
    }
}
