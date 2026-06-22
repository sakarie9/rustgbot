//! BBCode 解析器模块
//!
//! 将 NGA 的 BBCode 格式转换为 Telegram 支持的 HTML 格式。
//!
//! # 添加新标签
//!
//! ## 简单标签（无参数）
//! 只需在 `TAG_REGISTRY` 中添加一行即可。
//!
//! ## 带参数标签（如 `[tag=value]`）
//! 1. 在 `ParamTag` 枚举中添加变体
//! 2. 在 `BBCodeTag::parse_parameterized` 中添加解析逻辑
//! 3. 在 `ParamTag::base_name` 中添加映射
//! 4. 如需特殊渲染，在 `BBCodeParser::render_tag` 中添加处理

use regex::Regex;
use std::sync::OnceLock;

use crate::page::escape_html;
use crate::utils::{img_link_process, normalize_newlines, replace_html_entities};

// ============================================================================
// 标签注册表 - 添加简单标签只需在此处添加一行
// ============================================================================

/// 所有简单标签的定义
///
/// # 示例
/// ```ignore
/// TagDef::new("b", "<b>", "</b>"),         // 有 HTML 输出
/// TagDef::removed("img"),                   // 移除内容
/// TagDef::passthrough("flash"),             // 保留内容但无 HTML 包装
/// TagDef::passthrough("tr").with_close("\n"), // 自定义结束标签
/// ```
const TAG_REGISTRY: &[TagDef] = &[
    // 文本格式标签
    TagDef::new("b", "<b>", "</b>"),
    TagDef::new("i", "<i>", "</i>"),
    TagDef::new("u", "<u>", "</u>"),
    TagDef::new("s", "<s>", "</s>"),
    TagDef::new("del", "<del>", "</del>"),
    TagDef::new("quote", "<blockquote>", "</blockquote>"),
    // 媒体标签
    TagDef::removed("img"),
    TagDef::passthrough("flash"),
    // 结构标签
    TagDef::new("table", "\n<pre>", "</pre>"),
    TagDef::passthrough("tr").with_close("\n"),
    TagDef::passthrough("td").with_close(" │ "),
    // 引用标签（内容保留但标签移除）
    TagDef::passthrough("pid"),
    TagDef::passthrough("uid"),
    TagDef::passthrough("url"),
    TagDef::passthrough("collapse"),
    TagDef::passthrough("color"),
    TagDef::passthrough("h"),
    // 特殊标签
    TagDef::new("dice", "🎲 ", ""),
];

// ============================================================================
// 带参数标签 - 添加带参数标签需修改此处
// ============================================================================

/// 带参数的标签类型
///
/// 添加新的带参数标签：
/// 1. 在此枚举添加变体
/// 2. 在 `BBCodeTag::parse_parameterized` 添加解析
/// 3. 在 `base_name` 添加映射
#[derive(Debug, Clone, PartialEq)]
pub enum ParamTag {
    Url(String),
    Collapse(String),
    TableCell(String),
    Pid(String),
    Uid(String),
    Color(String),
    Sticker(String),
    Size(String),
    Align(String),
}

impl ParamTag {
    pub fn base_name(&self) -> &'static str {
        match self {
            Self::Url(_) => "url",
            Self::Collapse(_) => "collapse",
            Self::TableCell(_) => "td",
            Self::Pid(_) => "pid",
            Self::Uid(_) => "uid",
            Self::Color(_) => "color",
            Self::Sticker(_) => "s",
            Self::Size(_) => "size",
            Self::Align(_) => "align",
        }
    }
}

// ============================================================================
// 标签定义结构
// ============================================================================

/// 标签定义结构
struct TagDef {
    /// 标签名称（用于解析和匹配）
    name: &'static str,
    /// 开始 HTML 标签
    html_open: &'static str,
    /// 结束 HTML 标签
    html_close: &'static str,
    /// 是否移除内容
    remove_content: bool,
}

impl TagDef {
    /// 创建普通标签（有 HTML 输出）
    const fn new(name: &'static str, html_open: &'static str, html_close: &'static str) -> Self {
        Self {
            name,
            html_open,
            html_close,
            remove_content: false,
        }
    }

    /// 创建移除内容的标签（如图片）
    const fn removed(name: &'static str) -> Self {
        Self {
            name,
            html_open: "",
            html_close: "",
            remove_content: true,
        }
    }

    /// 创建透传标签（保留内容但无 HTML 包装）
    const fn passthrough(name: &'static str) -> Self {
        Self {
            name,
            html_open: "",
            html_close: "",
            remove_content: false,
        }
    }

    /// 设置自定义结束标签
    const fn with_close(mut self, close: &'static str) -> Self {
        self.html_close = close;
        self
    }
}

// ============================================================================
// BBCode 标签枚举
// ============================================================================

/// BBCode 标签类型
#[derive(Debug, Clone, PartialEq)]
pub enum BBCodeTag {
    /// 简单标签（通过 TAG_REGISTRY 定义）
    Simple(usize),
    /// 带参数的标签
    Parameterized(ParamTag),
}

impl BBCodeTag {
    /// 从标签名解析 BBCode 标签
    pub fn parse(tag: &str) -> Option<Self> {
        let lower = tag.to_lowercase();

        // 先尝试简单标签匹配
        for (idx, def) in TAG_REGISTRY.iter().enumerate() {
            if lower == def.name {
                return Some(Self::Simple(idx));
            }
        }

        // 尝试带参数的标签
        Self::parse_parameterized(tag)
    }

    /// 解析带参数的标签
    ///
    /// 添加新的带参数标签时，在此添加解析逻辑
    fn parse_parameterized(tag: &str) -> Option<Self> {
        let param_tag = if let Some(v) = tag.strip_prefix("url=") {
            ParamTag::Url(v.to_string())
        } else if let Some(v) = tag.strip_prefix("collapse=") {
            ParamTag::Collapse(v.to_string())
        } else if tag.starts_with("td") && tag.len() > 2 {
            ParamTag::TableCell(tag.strip_prefix("td").unwrap().to_string())
        } else if let Some(v) = tag.strip_prefix("pid=") {
            ParamTag::Pid(v.to_string())
        } else if let Some(v) = tag.strip_prefix("uid=") {
            ParamTag::Uid(v.to_string())
        } else if let Some(v) = tag.strip_prefix("color=") {
            ParamTag::Color(v.to_string())
        } else if tag.starts_with("s:ac:") || tag.starts_with("s:") {
            ParamTag::Sticker(tag.to_string())
        } else if let Some(v) = tag.strip_prefix("size=") {
            ParamTag::Size(v.to_string())
        } else if let Some(v) = tag.strip_prefix("align=") {
            ParamTag::Align(v.to_string())
        } else {
            return None;
        };
        Some(Self::Parameterized(param_tag))
    }

    /// 获取标签定义
    fn def(&self) -> Option<&'static TagDef> {
        match self {
            Self::Simple(idx) => TAG_REGISTRY.get(*idx),
            Self::Parameterized(p) => {
                let name = p.base_name();
                TAG_REGISTRY.iter().find(|d| d.name == name)
            }
        }
    }

    /// 获取标签的基本名称（用于匹配结束标签）
    pub fn base_name(&self) -> &'static str {
        match self {
            Self::Simple(idx) => TAG_REGISTRY.get(*idx).map_or("", |d| d.name),
            Self::Parameterized(p) => p.base_name(),
        }
    }

    /// 生成开始 HTML 标签
    pub fn to_html_open(&self) -> &'static str {
        self.def().map_or("", |d| d.html_open)
    }

    /// 生成结束 HTML 标签
    pub fn to_html_close(&self) -> &'static str {
        self.def().map_or("", |d| d.html_close)
    }

    /// 是否需要移除标签内容
    pub fn should_remove_content(&self) -> bool {
        match self {
            Self::Simple(idx) => TAG_REGISTRY.get(*idx).is_some_and(|d| d.remove_content),
            Self::Parameterized(ParamTag::Sticker(_)) => true,
            _ => false,
        }
    }

    /// 是否是自闭合标签
    pub fn is_self_closing(&self) -> bool {
        matches!(self, Self::Parameterized(ParamTag::Sticker(_)))
    }
}

// ============================================================================
// Rich Message BBCode 解析器
// ============================================================================

/// Rich Message 内容清理器
pub struct RichContentCleaner;

impl RichContentCleaner {
    /// 清理帖子内容为 Rich Message HTML
    pub fn clean(body: &str) -> String {
        let decoded = replace_html_entities(body);
        let parsed = RichBBCodeParser::new(&decoded).parse();
        normalize_newlines(&parsed)
    }
}

/// Rich Message BBCode 解析器
///
/// 将 NGA 的 BBCode 转换为 Telegram Rich Message HTML
pub struct RichBBCodeParser {
    chars: Vec<char>,
    pos: usize,
}

impl RichBBCodeParser {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    pub fn parse(&mut self) -> String {
        let mut result = String::new();
        while self.pos < self.chars.len() {
            if self.current_char() == '[' && self.peek_char() != '/' {
                self.process_tag(&mut result);
            } else {
                result.push(self.current_char());
                self.pos += 1;
            }
        }
        result
    }

    fn current_char(&self) -> char {
        self.chars.get(self.pos).copied().unwrap_or('\0')
    }

    fn peek_char(&self) -> char {
        self.chars.get(self.pos + 1).copied().unwrap_or('\0')
    }

    fn process_tag(&mut self, result: &mut String) {
        if let Some((tag, tag_end)) = self.parse_opening_tag_at(self.pos) {
            self.pos = tag_end;

            if tag.is_self_closing() {
                if !tag.should_remove_content() {
                    result.push_str(tag.to_html_open());
                    result.push_str(tag.to_html_close());
                }
                return;
            }

            if let Some(content_end) = self.find_closing_tag(&tag) {
                let content = self.extract_content(self.pos, content_end);

                if tag.should_remove_content() {
                    // [img] → <img/>
                    if tag.base_name() == "img" {
                        let img_url = img_link_process(&content);
                        result.push_str(&format!("<img src=\"{}\"/>", img_url));
                    }
                    self.skip_closing_tag_at(content_end);
                    return;
                }

                self.render_tag(&tag, &content, result);
                self.skip_closing_tag_at(content_end);
            } else {
                result.push('[');
            }
        } else {
            result.push(self.current_char());
            self.pos += 1;
        }
    }

    fn render_tag(&self, tag: &BBCodeTag, content: &str, result: &mut String) {
        match tag {
            // 表格 → <table>（前后加段落分隔）
            _ if tag.base_name() == "table" => {
                result.push_str(&format!("\n\n{}\n\n", self.format_rich_table(content)));
                return;
            }
            // [url=href] → <a>
            BBCodeTag::Parameterized(ParamTag::Url(href)) => {
                let processed = Self::new(content).parse();
                result.push_str(&format!(
                    "<a href=\"{}\">{}</a>",
                    escape_html_attr(href),
                    processed
                ));
                return;
            }
            // [collapse=title] → <details>（前后加段落分隔）
            BBCodeTag::Parameterized(ParamTag::Collapse(title)) => {
                let processed = Self::new(content).parse();
                result.push_str(&format!(
                    "\n\n<details><summary>{}</summary>{}</details>\n\n",
                    escape_html(title),
                    processed
                ));
                return;
            }
            // [size=N] → <b>
            BBCodeTag::Parameterized(ParamTag::Size(_)) => {
                let processed = Self::new(content).parse();
                result.push_str(&format!("<b>{}</b>", processed));
                return;
            }
            // [color]/[pid]/[uid]/[align] → 直接输出内容
            BBCodeTag::Parameterized(ParamTag::Color(_))
            | BBCodeTag::Parameterized(ParamTag::Pid(_))
            | BBCodeTag::Parameterized(ParamTag::Uid(_))
            | BBCodeTag::Parameterized(ParamTag::Align(_)) => {
                result.push_str(&Self::new(content).parse());
                return;
            }
            // 贴纸 → 移除
            BBCodeTag::Parameterized(ParamTag::Sticker(_)) => return,
            // 表格单元格（由 format_rich_table 处理）
            BBCodeTag::Parameterized(ParamTag::TableCell(_)) => {
                result.push_str(&Self::new(content).parse());
                return;
            }
            _ => {}
        }

        // 无参数 url → <a>
        if tag.base_name() == "url" {
            let processed = Self::new(content).parse();
            result.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                escape_html_attr(&processed),
                processed
            ));
            return;
        }

        // [quote] → <blockquote>（前后加段落分隔）
        if tag.base_name() == "quote" {
            let processed = Self::new(content).parse();
            result.push_str(&format!("\n\n<blockquote>{}</blockquote>\n\n", processed));
            return;
        }

        // 普通标签
        let processed = Self::new(content).parse();
        result.push_str(tag.to_html_open());
        result.push_str(&processed);
        result.push_str(tag.to_html_close());
    }

    /// 格式化 Rich Message 表格
    fn format_rich_table(&self, content: &str) -> String {
        static TR_REGEX: OnceLock<Regex> = OnceLock::new();
        static TD_REGEX: OnceLock<Regex> = OnceLock::new();

        let tr_pattern = TR_REGEX.get_or_init(|| Regex::new(r"(?s)\[tr\](.*?)\[/tr\]").unwrap());
        let td_pattern =
            TD_REGEX.get_or_init(|| Regex::new(r"(?s)\[td[^]]*\](.*?)\[/td\]").unwrap());

        let rows: Vec<Vec<String>> = tr_pattern
            .find_iter(content)
            .filter_map(|tr_match| {
                let cells: Vec<String> = td_pattern
                    .captures_iter(tr_match.as_str())
                    .map(|cap| {
                        let cell_content = cap.get(1).map_or("", |m| m.as_str());
                        Self::new(cell_content).parse().trim().to_string()
                    })
                    .collect();
                if cells.is_empty() { None } else { Some(cells) }
            })
            .collect();

        if rows.is_empty() {
            return String::new();
        }

        let mut html = String::from("<table>");
        for (i, row) in rows.iter().enumerate() {
            html.push_str("<tr>");
            for cell in row {
                if i == 0 {
                    html.push_str(&format!("<td><b>{}</b></td>", cell));
                } else {
                    html.push_str(&format!("<td>{}</td>", cell));
                }
            }
            html.push_str("</tr>");
        }
        html.push_str("</table>");
        html
    }

    // ========== 辅助方法 ==========

    fn parse_opening_tag_at(&self, start: usize) -> Option<(BBCodeTag, usize)> {
        if start >= self.chars.len() || self.chars[start] != '[' {
            return None;
        }
        let end = (start + 1..self.chars.len()).find(|&i| self.chars[i] == ']')?;
        let tag_content: String = self.chars[start + 1..end].iter().collect();
        BBCodeTag::parse(&tag_content).map(|tag| (tag, end + 1))
    }

    fn find_closing_tag(&self, tag: &BBCodeTag) -> Option<usize> {
        let tag_name = tag.base_name();
        let mut pos = self.pos;
        let mut depth = 1;
        while pos < self.chars.len() {
            if self.chars[pos] == '[' {
                if self.is_closing_tag_at(pos, tag_name) {
                    depth -= 1;
                    if depth == 0 {
                        return Some(pos);
                    }
                } else if self.is_same_opening_tag_at(pos, tag_name) {
                    depth += 1;
                }
            }
            pos += 1;
        }
        None
    }

    fn is_closing_tag_at(&self, pos: usize, expected: &str) -> bool {
        if pos + 2 >= self.chars.len() {
            return false;
        }
        if self.chars[pos] != '[' || self.chars[pos + 1] != '/' {
            return false;
        }
        let end = (pos + 2..self.chars.len()).find(|&i| self.chars[i] == ']');
        if let Some(end) = end {
            let tag_content: String = self.chars[pos + 2..end].iter().collect();
            tag_content.eq_ignore_ascii_case(expected)
        } else {
            false
        }
    }

    fn is_same_opening_tag_at(&self, pos: usize, expected: &str) -> bool {
        if let Some((tag, _)) = self.parse_opening_tag_at(pos) {
            tag.base_name() == expected
        } else {
            false
        }
    }

    fn extract_content(&self, start: usize, end: usize) -> String {
        self.chars[start..end].iter().collect()
    }

    fn skip_closing_tag_at(&mut self, pos: usize) {
        self.pos = pos;
        if self.pos < self.chars.len() && self.chars[self.pos] == '[' {
            while self.pos < self.chars.len() && self.chars[self.pos] != ']' {
                self.pos += 1;
            }
            if self.pos < self.chars.len() {
                self.pos += 1;
            }
        }
    }
}

/// 转义 HTML 属性值
fn escape_html_attr(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
