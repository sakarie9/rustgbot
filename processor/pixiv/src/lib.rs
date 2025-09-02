use common::{LinkProcessor, ProcessorError, ProcessorResult, ProcessorResultType};
use regex::Regex;
use std::sync::OnceLock;

mod api;
pub mod constants;
mod models;
mod processor;
mod tests;
mod utils;

use processor::get_pixiv;

static PIXIV_REGEX: OnceLock<Regex> = OnceLock::new();

/// Pixiv链接处理器
pub struct PixivLinkProcessor;

impl PixivLinkProcessor {
    const PATTERN: &'static str = r"(?:https?://)?(?:www\.)?pixiv\.net/artworks/(\d+)(?:\?p=\d+)?";
}

#[async_trait::async_trait]
impl LinkProcessor for PixivLinkProcessor {
    fn pattern(&self) -> &'static str {
        Self::PATTERN
    }

    fn regex(&self) -> &Regex {
        PIXIV_REGEX.get_or_init(|| Regex::new(Self::PATTERN).expect("Invalid Pixiv regex pattern"))
    }

    async fn process_captures(&self, captures: &regex::Captures<'_>) -> ProcessorResultType {
        if let Some(id_match) = captures.get(1) {
            let id = id_match.as_str();
            match get_pixiv(id).await {
                Ok(parsed) => {
                    if parsed.urls.is_empty() {
                        return Ok(ProcessorResult::Text(parsed.caption));
                    }
                    Ok(ProcessorResult::Media(parsed))
                }
                Err(e) => Err(ProcessorError::with_source(
                    "处理Pixiv链接失败",
                    e.to_string(),
                )),
            }
        } else {
            Err(ProcessorError::new("无法从Pixiv链接中提取作品ID"))
        }
    }

    fn name(&self) -> &'static str {
        "Pixiv"
    }
}
