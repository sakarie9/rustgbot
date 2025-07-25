use regex::Regex;
use scraper::{Html, Selector};
use std::sync::OnceLock;

use crate::utils::{
    get_nga_cookie, get_nga_img_links, normalize_newlines, preprocess_url, replace_html_entities,
    substring_desc, NGA_UA,
};
use common::{LinkProcessor, ProcessorError, ProcessorResult, ProcessorResultType};

mod tests;
mod utils;

static NGA_REGEX: OnceLock<Regex> = OnceLock::new();

/// NGA链接处理器
pub struct NGALinkProcessor;

impl NGALinkProcessor {
    const PATTERN: &'static str = r"(?:https?://(?:bbs\.nga\.cn|ngabbs\.com|nga\.178\.com|bbs\.gnacn\.cc)[-a-zA-Z0-9@:%_\+.~#?&//=]*)";
}

#[async_trait::async_trait]
impl LinkProcessor for NGALinkProcessor {
    fn pattern(&self) -> &'static str {
        Self::PATTERN
    }

    fn regex(&self) -> &Regex {
        NGA_REGEX.get_or_init(|| Regex::new(Self::PATTERN).expect("Invalid NGA regex pattern"))
    }

    async fn process_captures(&self, captures: &regex::Captures<'_>) -> ProcessorResultType {
        let full_match = captures.get(0).unwrap().as_str();
        match NGAFetcher::parse(full_match).await {
            Ok(parsed) => Ok(ProcessorResult::Media(parsed)),
            Err(e) => Err(ProcessorError::with_source(
                "处理NGA链接失败",
                e.to_string(),
            )),
        }
    }

    fn name(&self) -> &'static str {
        "NGA"
    }
}

/// NGA 模块的错误类型
#[derive(Debug)]
enum NGAError {
    NetworkError(reqwest::Error),
    ParseError(String),
    HttpError { status: u16, message: String },
}

impl std::fmt::Display for NGAError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NGAError::NetworkError(e) => write!(f, "网络请求失败: {}", e),
            NGAError::ParseError(msg) => write!(f, "解析页面失败: {}", msg),
            NGAError::HttpError { status, message } => {
                write!(f, "HTTP 错误 {}: {}", status, message)
            }
        }
    }
}

impl std::error::Error for NGAError {}

impl From<reqwest::Error> for NGAError {
    fn from(error: reqwest::Error) -> Self {
        NGAError::NetworkError(error)
    }
}

impl From<anyhow::Error> for NGAError {
    fn from(error: anyhow::Error) -> Self {
        NGAError::ParseError(error.to_string())
    }
}

/// NGA 模块的结果类型
type NGAResult<T> = std::result::Result<T, NGAError>;

/// NGA 页面数据结构
#[derive(Debug, Clone)]
struct NGAPage {
    url: String,
    title: String,
    content: String, // 直接存储清理后的内容
    images: Vec<String>,
}

/// NGA 抓取器的主要公共接口
struct NGAFetcher;

impl NGAFetcher {
    /// 解析
    async fn parse(url: &str) -> NGAResult<common::ProcessorResultMedia> {
        let processed_url = preprocess_url(url);
        let page = Self::fetch_page(&processed_url).await?;
        let text = get_summary(&page);
        let urls = page.images;
        Ok(common::ProcessorResultMedia {
            caption: text,
            urls,
        })
    }

    /// 获取并解析 NGA 页面
    async fn fetch_page(url: &str) -> NGAResult<NGAPage> {
        let html = Self::fetch_html(url).await?;
        Self::parse_page(url, &html)
    }

    /// 仅获取 HTML 内容
    async fn fetch_html(url: &str) -> NGAResult<String> {
        get_nga_html(url).await
    }

    /// 仅解析 HTML 内容
    fn parse_page(url: &str, html: &str) -> NGAResult<NGAPage> {
        parse_nga_page(url, html)
            .ok_or_else(|| NGAError::ParseError("Failed to parse NGA page".to_string()))
    }
}

fn get_summary(page: &NGAPage) -> String {
    let title = format!(
        "<b><u><a href=\"{}\">{}</a></u></b>",
        page.url,
        page.title.trim()
    );
    let truncated_content = substring_desc(&page.content);

    let summary = format!("{}\n\n{}", title, truncated_content);

    // 日志输出（仅在调试时）
    #[cfg(debug_assertions)]
    {
        println!("Summary:\n{}", summary);
    }

    summary
}

async fn get_nga_html(url: &str) -> NGAResult<String> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", NGA_UA)
        .header("Cookie", get_nga_cookie())
        .send()
        .await?;

    let status = response.status();

    if status.is_success() {
        response
            .text_with_charset("gbk")
            .await
            .map_err(NGAError::from)
    } else {
        // 根据不同的HTTP状态码提供更具体的错误信息
        let status_code = status.as_u16();
        let error_message = match status_code {
            403 => "此帖子被锁定或无访问权限".to_string(),
            _ => format!("HTTP 请求失败，状态码: {}", status_code),
        };

        Err(NGAError::HttpError {
            status: status_code,
            message: error_message,
        })
    }
}

fn parse_nga_page(url: &str, html: &str) -> Option<NGAPage> {
    // 将 HTML 片段解析为文档
    let document = Html::parse_document(html);

    // 创建 CSS 选择器来定位标题和内容
    // #postsubject0 选择 id 为 "postsubject0" 的元素
    let title_selector =
        Selector::parse("h3#postsubject0").expect("Failed to parse title selector");
    // #postcontent0 选择 id 为 "postcontent0" 的元素
    let content_selector =
        Selector::parse("p#postcontent0").expect("Failed to parse content selector");

    // 查找并提取标题文本
    let title = document
        .select(&title_selector)
        .next()
        .map(|element| element.text().collect::<String>());

    // 查找并提取内容文本
    let content = document.select(&content_selector).next().map(|element| {
        // 获取内部HTML，保留 <br/> 标签
        element.inner_html()
    });

    if title.is_none() || content.is_none() {
        return None; // 如果没有找到标题或内容，返回 None
    }

    let title = title.unwrap_or_default();
    let content = content.unwrap_or_default();

    // 提取图片链接（从原始内容中提取）
    let image_links = get_nga_img_links(&content);

    // 清理内容
    let cleaned_content = clean_body(&content);

    // 日志输出（仅在调试时）
    #[cfg(debug_assertions)]
    {
        println!("--- 提取结果 ---");
        println!("标题: {}", title.trim());
        println!("原始内容:\n{}", content.trim());
        println!("清理内容:\n{}", cleaned_content.trim());
        println!("--- 提取到的图片链接 🖼️ ---");
        for link in &image_links {
            println!("{}", link);
        }
    }

    // 这里返回实际解析的内容和图片链接
    Some(NGAPage {
        url: url.to_string(),
        title: title.trim().to_string(),
        content: cleaned_content, // 直接使用清理后的内容
        images: image_links,
    })
}

fn clean_body(body: &str) -> String {
    // 第一步：处理 HTML 实体和标签
    let step1 = replace_html_entities(body);

    // 第二步：移除HTML标签但保留文本内容
    // let step2 = remove_html_tags(&step1);

    // 第三步：使用新的 BBCode 解析器处理标签
    let mut parser = BBCodeParser::new(&step1);
    let step3 = parser.parse();

    // 第四步：规范化换行符
    normalize_newlines(&step3)
}

// BBCode 解析器模块
/// BBCode 标签定义
#[derive(Debug, Clone, PartialEq)]
enum BBCodeTag {
    Bold,
    Italic,
    Underline,
    Strike,
    Delete,
    Quote,
    Url(Option<String>), // URL 可能有参数
    Img,
    Collapse(String),          // 折叠标签有标题
    Sticker(String),           // 表情标签有类型
    Table,                     // 表格标签
    TableRow,                  // 表格行标签
    TableCell(Option<String>), // 表格单元格标签，可能有宽度参数如td40
}

impl BBCodeTag {
    fn from_tag_name(tag: &str) -> Option<Self> {
        match tag.to_lowercase().as_str() {
            "b" => Some(BBCodeTag::Bold),
            "i" => Some(BBCodeTag::Italic),
            "u" => Some(BBCodeTag::Underline),
            "s" => Some(BBCodeTag::Strike),
            "del" => Some(BBCodeTag::Delete),
            "quote" => Some(BBCodeTag::Quote),
            "url" => Some(BBCodeTag::Url(None)),
            "img" => Some(BBCodeTag::Img),
            "table" => Some(BBCodeTag::Table),
            "tr" => Some(BBCodeTag::TableRow),
            "td" => Some(BBCodeTag::TableCell(None)),
            _ => {
                if tag.starts_with("url=") {
                    // 处理带参数的URL标签，如 [url=https://x.com]
                    let url = tag.strip_prefix("url=").unwrap_or("").to_string();
                    Some(BBCodeTag::Url(Some(url)))
                } else if tag.starts_with("collapse=") {
                    let title = tag.strip_prefix("collapse=").unwrap_or("").to_string();
                    Some(BBCodeTag::Collapse(title))
                } else if tag.starts_with("td") && tag.len() > 2 {
                    // 处理带宽度参数的表格单元格，如 td40
                    let width = tag.strip_prefix("td").unwrap_or("").to_string();
                    Some(BBCodeTag::TableCell(Some(width)))
                } else if tag.starts_with("s:ac:") || tag.starts_with("s:") {
                    // 表情标签，如 s:ac:赞同, s:ac:cry 等
                    Some(BBCodeTag::Sticker(tag.to_string()))
                } else {
                    None
                }
            }
        }
    }

    fn to_html_open(&self) -> String {
        match self {
            BBCodeTag::Bold => "<b>".to_string(),
            BBCodeTag::Italic => "<i>".to_string(),
            BBCodeTag::Underline => "<u>".to_string(),
            BBCodeTag::Strike => "<s>".to_string(),
            BBCodeTag::Delete => "<del>".to_string(),
            BBCodeTag::Quote => "".to_string(), // Quote 标签被移除
            BBCodeTag::Url(url) => {
                if let Some(href) = url {
                    format!("<a href=\"{}\">", href)
                } else {
                    "<a href=\"".to_string() // 将在内容中填充 URL
                }
            }
            BBCodeTag::Img => "".to_string(), // 图片标签被移除
            BBCodeTag::Collapse(title) => format!("[{}] ", title),
            BBCodeTag::Sticker(_) => "".to_string(), // 表情标签被移除
            BBCodeTag::Table => "\n<pre>".to_string(), // 使用 <pre> 标签包裹表格内容
            BBCodeTag::TableRow => "".to_string(),
            BBCodeTag::TableCell(_) => "".to_string(),
        }
    }

    fn to_html_close(&self) -> String {
        match self {
            BBCodeTag::Bold => "</b>".to_string(),
            BBCodeTag::Italic => "</i>".to_string(),
            BBCodeTag::Underline => "</u>".to_string(),
            BBCodeTag::Strike => "</s>".to_string(),
            BBCodeTag::Delete => "</del>".to_string(),
            BBCodeTag::Quote => "".to_string(),
            BBCodeTag::Url(_) => "</a>".to_string(),
            BBCodeTag::Img => "".to_string(),
            BBCodeTag::Collapse(title) => format!(" [/{}]", title),
            BBCodeTag::Sticker(_) => "".to_string(),
            BBCodeTag::Table => "</pre>".to_string(),
            BBCodeTag::TableRow => "\n".to_string(),
            BBCodeTag::TableCell(_) => " │ ".to_string(),
        }
    }

    fn should_remove_content(&self) -> bool {
        matches!(self, BBCodeTag::Img | BBCodeTag::Sticker(_))
    }

    fn is_self_closing(&self) -> bool {
        matches!(self, BBCodeTag::Sticker(_))
    }
}

// BBCode 解析器
struct BBCodeParser {
    input: Vec<char>,
    position: usize,
}

impl BBCodeParser {
    fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
        }
    }

    fn parse(&mut self) -> String {
        let mut result = String::new();

        while self.position < self.input.len() {
            if self.current_char() == '[' && self.peek_char() != '/' {
                // 尝试解析开始标签
                if let Some((tag, tag_end)) = self.parse_opening_tag() {
                    self.position = tag_end;

                    // 检查是否是自闭合标签（如表情）
                    if tag.is_self_closing() {
                        // 自闭合标签，如果需要移除内容则跳过，否则添加 HTML
                        if !tag.should_remove_content() {
                            result.push_str(&tag.to_html_open());
                            result.push_str(&tag.to_html_close());
                        }
                        continue;
                    }

                    let content_start = self.position;

                    // 查找匹配的结束标签
                    if let Some(content_end) = self.find_matching_closing_tag(&tag, content_start) {
                        let content = self.input[content_start..content_end]
                            .iter()
                            .collect::<String>();

                        if tag.should_remove_content() {
                            // 对于需要移除内容的标签（如图片），跳过整个标签
                            self.position = self.skip_closing_tag(content_end);
                            continue;
                        }

                        // 特殊处理表格标签
                        if matches!(tag, BBCodeTag::Table) {
                            let formatted_table = self.format_table(&content);
                            result.push_str(&tag.to_html_open());
                            result.push_str(&formatted_table);
                            result.push_str(&tag.to_html_close());
                            self.position = self.skip_closing_tag(content_end);
                            continue;
                        }

                        // 递归处理标签内容
                        let mut inner_parser = BBCodeParser::new(&content);
                        let processed_content = inner_parser.parse();

                        // 生成 HTML
                        result.push_str(&tag.to_html_open());

                        // 特殊处理 URL 标签
                        match &tag {
                            BBCodeTag::Url(Some(_)) => {
                                // 带参数的URL：[url=https://x.com]推特[/url]
                                result.push_str(&processed_content);
                            }
                            BBCodeTag::Url(None) => {
                                // 不带参数的URL：[url]https://x.com[/url]
                                result.push_str(&processed_content);
                                result.push_str("\">");
                                result.push_str(&processed_content);
                            }
                            _ => {
                                result.push_str(&processed_content);
                            }
                        }

                        result.push_str(&tag.to_html_close());

                        // 移动到结束标签之后
                        self.position = self.skip_closing_tag(content_end);
                    } else {
                        // 没有找到匹配的结束标签，回退并当作普通文本处理
                        self.position -= tag_end - self.position;
                        result.push(self.current_char());
                        self.position += 1;
                    }
                } else {
                    // 不是有效的标签，当作普通文本处理
                    result.push(self.current_char());
                    self.position += 1;
                }
            } else {
                // 普通文本或结束标签
                result.push(self.current_char());
                self.position += 1;
            }
        }

        result
    }

    fn current_char(&self) -> char {
        self.input.get(self.position).copied().unwrap_or('\0')
    }

    fn peek_char(&self) -> char {
        self.input.get(self.position + 1).copied().unwrap_or('\0')
    }

    fn parse_opening_tag(&self) -> Option<(BBCodeTag, usize)> {
        if self.current_char() != '[' {
            return None;
        }

        let mut tag_end = self.position + 1;
        while tag_end < self.input.len() && self.input[tag_end] != ']' {
            tag_end += 1;
        }

        if tag_end >= self.input.len() {
            return None; // 没有找到结束的 ]
        }

        let tag_content = self.input[self.position + 1..tag_end]
            .iter()
            .collect::<String>();

        if let Some(tag) = BBCodeTag::from_tag_name(&tag_content) {
            Some((tag, tag_end + 1))
        } else {
            None
        }
    }

    fn find_matching_closing_tag(&self, tag: &BBCodeTag, start: usize) -> Option<usize> {
        let tag_name = self.get_tag_name(tag);
        let mut pos = start;
        let mut depth = 1;

        while pos < self.input.len() {
            if self.input[pos] == '[' {
                if pos + 1 < self.input.len() && self.input[pos + 1] == '/' {
                    // 这是一个结束标签
                    if let Some(end_pos) = self.parse_closing_tag_at(pos, &tag_name) {
                        depth -= 1;
                        if depth == 0 {
                            return Some(pos);
                        }
                        pos = end_pos;
                        continue;
                    }
                } else {
                    // 这可能是一个开始标签
                    if let Some((inner_tag, _)) = self.parse_opening_tag_at(pos) {
                        if self.get_tag_name(&inner_tag) == tag_name {
                            depth += 1;
                        }
                    }
                }
            }
            pos += 1;
        }

        None
    }

    fn parse_opening_tag_at(&self, pos: usize) -> Option<(BBCodeTag, usize)> {
        if pos >= self.input.len() || self.input[pos] != '[' {
            return None;
        }

        let mut tag_end = pos + 1;
        while tag_end < self.input.len() && self.input[tag_end] != ']' {
            tag_end += 1;
        }

        if tag_end >= self.input.len() {
            return None;
        }

        let tag_content = self.input[pos + 1..tag_end].iter().collect::<String>();

        if let Some(tag) = BBCodeTag::from_tag_name(&tag_content) {
            Some((tag, tag_end + 1))
        } else {
            None
        }
    }

    fn parse_closing_tag_at(&self, pos: usize, expected_tag: &str) -> Option<usize> {
        if pos + 1 >= self.input.len() || self.input[pos] != '[' || self.input[pos + 1] != '/' {
            return None;
        }

        let mut tag_end = pos + 2;
        while tag_end < self.input.len() && self.input[tag_end] != ']' {
            tag_end += 1;
        }

        if tag_end >= self.input.len() {
            return None;
        }

        let tag_content = self.input[pos + 2..tag_end].iter().collect::<String>();

        if tag_content.to_lowercase() == expected_tag.to_lowercase() {
            Some(tag_end + 1)
        } else {
            None
        }
    }

    fn skip_closing_tag(&self, pos: usize) -> usize {
        let mut current = pos;
        if current < self.input.len() && self.input[current] == '[' {
            while current < self.input.len() && self.input[current] != ']' {
                current += 1;
            }
            if current < self.input.len() {
                current += 1; // 跳过 ]
            }
        }
        current
    }

    fn get_tag_name(&self, tag: &BBCodeTag) -> String {
        match tag {
            BBCodeTag::Bold => "b".to_string(),
            BBCodeTag::Italic => "i".to_string(),
            BBCodeTag::Underline => "u".to_string(),
            BBCodeTag::Strike => "s".to_string(),
            BBCodeTag::Delete => "del".to_string(),
            BBCodeTag::Quote => "quote".to_string(),
            BBCodeTag::Url(_) => "url".to_string(),
            BBCodeTag::Img => "img".to_string(),
            BBCodeTag::Collapse(_) => "collapse".to_string(),
            BBCodeTag::Sticker(_) => "s".to_string(),
            BBCodeTag::Table => "table".to_string(),
            BBCodeTag::TableRow => "tr".to_string(),
            BBCodeTag::TableCell(_) => "td".to_string(),
        }
    }

    fn format_table(&self, content: &str) -> String {
        use std::sync::OnceLock;
        use tabled::{Table, settings::Style};

        // 先快速检查是否包含表格标签，如果不包含直接返回
        if !content.contains("[tr]") || !content.contains("[td") {
            return String::new();
        }

        // 使用 OnceLock 固化正则表达式，避免重复编译
        static TR_PATTERN: OnceLock<regex::Regex> = OnceLock::new();
        static TD_PATTERN: OnceLock<regex::Regex> = OnceLock::new();

        let tr_pattern =
            TR_PATTERN.get_or_init(|| regex::Regex::new(r"(?s)\[tr\](.*?)\[/tr\]").unwrap());

        let td_pattern =
            TD_PATTERN.get_or_init(|| regex::Regex::new(r"(?s)\[td[^]]*\](.*?)\[/td\]").unwrap());

        let mut rows = Vec::new();

        // 提取所有表格行
        for tr_match in tr_pattern.find_iter(content) {
            let row_content = tr_match.as_str();
            let mut cells = Vec::new();

            // 提取行中的所有单元格
            for td_match in td_pattern.find_iter(row_content) {
                let cell_content = td_pattern.replace(td_match.as_str(), "$1");
                // 递归处理单元格内容中可能的BBCode
                let mut cell_parser = BBCodeParser::new(&cell_content);
                let processed_cell = cell_parser.parse();
                cells.push(processed_cell.trim().to_string());
            }

            if !cells.is_empty() {
                rows.push(cells);
            }
        }

        if rows.is_empty() {
            return String::new();
        }

        // 使用 tabled 创建表格
        let mut table = Table::from_iter(rows);
        table.with(Style::empty());
        table.to_string()
    }
}
