//! NGA 页面数据结构

use scraper::{Html, Selector};

use crate::bbcode::RichContentCleaner;

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
    /// 原始 BBCode 内容（用于生成 Rich Message）
    raw_content: String,
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

        #[cfg(debug_assertions)]
        Self::debug_output(&title, &raw_content);

        Some(Self {
            url: url.to_string(),
            title,
            raw_content: raw_content.to_string(),
        })
    }

    /// 生成 Rich Message HTML
    ///
    /// 使用 Rich Message 格式保留原始帖子布局
    pub fn to_rich_html(&self) -> String {
        let rich_content = RichContentCleaner::clean(&self.raw_content);

        // 仅在标题非空时生成标题块
        let title_block = if !self.title.trim().is_empty() {
            let escaped_title = escape_html(&self.title);
            format!("<h3><a href=\"{}\">{}</a></h3>", self.url, escaped_title)
        } else {
            String::new()
        };

        // 将连续换行转为段落分隔，单换行转为 <br/>
        // Telegram 的 rich message 解析器会自动识别块级标签
        let content = rich_content
            .split("\n\n")
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                let trimmed = s.trim();
                // 如果已经是块级标签开头，直接保留
                if trimmed.starts_with('<')
                    && (trimmed.starts_with("<blockquote")
                        || trimmed.starts_with("<table")
                        || trimmed.starts_with("<details")
                        || trimmed.starts_with("<pre")
                        || trimmed.starts_with("<h")
                        || trimmed.starts_with("<hr")
                        || trimmed.starts_with("<img"))
                {
                    trimmed.to_string()
                } else {
                    let with_br = trimmed.replace('\n', "<br/>");
                    format!("<p>{}</p>", with_br)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!("{}{}", title_block, content)
    }

    #[cfg(debug_assertions)]
    fn debug_output(title: &str, raw: &str) {
        println!("--- 提取结果 ---");
        println!("标题: {}", title);
        println!("原始内容:\n{}", raw.trim());
    }
}
