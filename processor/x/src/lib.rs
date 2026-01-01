use common::{LinkProcessor, ProcessorError, ProcessorResult, ProcessorResultType};
use regex::Regex;
use std::sync::OnceLock;

static X_REGEX: OnceLock<Regex> = OnceLock::new();

/// X/Twitter链接处理器
pub struct XLinkProcessor;

impl XLinkProcessor {
    const PATTERN: &'static str =
        r"(?:https?://)?\b(?:x\.com|(?:www\.|vx)?twitter\.com)/(\w+)/status/(\d+)";
}

#[async_trait::async_trait]
impl LinkProcessor for XLinkProcessor {
    fn pattern(&self) -> &'static str {
        Self::PATTERN
    }

    fn regex(&self) -> &Regex {
        X_REGEX.get_or_init(|| Regex::new(Self::PATTERN).expect("Invalid X regex pattern"))
    }

    async fn process_captures(&self, captures: &regex::Captures<'_>) -> ProcessorResultType {
        if captures.len() >= 3 {
            let username = &captures[1];
            let status_id = &captures[2];

            log::debug!(
                "X link details - Username: {}, Status ID: {}",
                username,
                status_id
            );

            let processed = format!("https://fxtwitter.com/{}/status/{}", username, status_id);
            Ok(ProcessorResult::Text(processed))
        } else {
            Err(ProcessorError::new("无法解析X链接"))
        }
    }

    fn name(&self) -> &'static str {
        "X/Twitter"
    }
}
