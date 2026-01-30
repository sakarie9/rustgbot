//! NGA 页面数据结构

use scraper::{Html, Selector};

use crate::bbcode::ContentCleaner;
use crate::utils::get_nga_img_links;
use common::substring_desc;

/// 转义 HTML 特殊字符，防止 Telegram 将文本内容识别为 HTML 标签
pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// NGA 页面数据
#[derive(Debug, Clone)]
pub struct NGAPage {
    pub url: String,
    pub title: String,
    /// 已清理的帖子内容（HTML 格式）
    pub content: String,
    /// 提取的图片链接列表
    pub images: Vec<String>,
}

impl NGAPage {
    /// 从 HTML 解析页面数据
    pub fn from_html(url: &str, html: &str) -> Option<Self> {
        let document = Html::parse_document(html);

        // 提取标题
        let title_selector = Selector::parse("h3#postsubject0").ok()?;
        let title = document
            .select(&title_selector)
            .next()?
            .text()
            .collect::<String>()
            .trim()
            .to_string();

        // 提取内容
        let content_selector = Selector::parse("p#postcontent0").ok()?;
        let raw_content = document.select(&content_selector).next()?.inner_html();

        // 提取图片链接（从原始内容提取）
        let images = get_nga_img_links(&raw_content);

        // 清理内容
        let content = ContentCleaner::clean(&raw_content);

        #[cfg(debug_assertions)]
        Self::debug_output(&title, &raw_content, &content, &images);

        Some(Self {
            url: url.to_string(),
            title,
            content,
            images,
        })
    }

    /// 生成摘要文本
    pub fn to_summary(&self) -> String {
        let escaped_title = escape_html(self.title.trim());
        let title_html = format!(
            "<b><u><a href=\"{}\">{}</a></u></b>",
            self.url, escaped_title
        );
        let truncated_content = substring_desc(&self.content);

        let summary = format!("{}\n\n{}", title_html, truncated_content);

        #[cfg(debug_assertions)]
        println!("Summary:\n{}", summary);

        summary
    }

    #[cfg(debug_assertions)]
    fn debug_output(title: &str, raw: &str, cleaned: &str, images: &[String]) {
        println!("--- 提取结果 ---");
        println!("标题: {}", title);
        println!("原始内容:\n{}", raw.trim());
        println!("清理内容:\n{}", cleaned.trim());
        println!("--- 提取到的图片链接 🖼️ ---");
        for link in images {
            println!("{}", link);
        }
    }
}
