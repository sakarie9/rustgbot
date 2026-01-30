//! NGA 论坛链接处理模块
//!
//! 本模块负责解析 NGA 论坛链接，提取帖子标题、内容和图片，
//! 并将 BBCode 格式转换为 Telegram 支持的 HTML 格式。
//!
//! # 模块结构
//!
//! - [`bbcode`] - BBCode 解析器（添加新标签请查看此模块）
//! - [`error`] - 错误类型定义
//! - [`fetcher`] - 页面抓取器
//! - [`page`] - 页面数据结构
//! - [`utils`] - 工具函数

use regex::Regex;
use std::sync::OnceLock;

use common::{LinkProcessor, ProcessorError, ProcessorResult, ProcessorResultType};

pub mod bbcode;
mod error;
mod fetcher;
mod page;
mod tests;
mod utils;

pub use bbcode::{BBCodeParser, ContentCleaner};
pub use error::{NGAError, NGAResult};
pub use fetcher::NGAFetcher;
pub use page::NGAPage;

// ============================================================================
// 链接处理器
// ============================================================================

static NGA_REGEX: OnceLock<Regex> = OnceLock::new();

/// NGA 链接处理器
///
/// 支持以下域名：
/// - bbs.nga.cn
/// - ngabbs.com
/// - nga.178.com
/// - bbs.gnacn.cc
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
        let url = captures.get(0).unwrap().as_str();
        NGAFetcher::parse(url)
            .await
            .map(ProcessorResult::Media)
            .map_err(|e| ProcessorError::with_source("处理NGA链接失败", e.to_string()))
    }

    fn name(&self) -> &'static str {
        "NGA"
    }
}

// ============================================================================
// 向后兼容函数（仅供测试使用）
// ============================================================================

#[cfg(test)]
fn clean_body(body: &str) -> String {
    ContentCleaner::clean(body)
}

#[cfg(test)]
fn parse_nga_page(url: &str, html: &str) -> Option<NGAPage> {
    NGAPage::from_html(url, html)
}

#[cfg(test)]
fn get_summary(page: &NGAPage) -> String {
    page.to_summary()
}

#[cfg(test)]
async fn get_nga_html(url: &str) -> NGAResult<String> {
    NGAFetcher::fetch_html(url).await
}
