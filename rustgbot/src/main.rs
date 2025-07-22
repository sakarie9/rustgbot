use dotenv::dotenv;
use regex::Regex;
use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;
use teloxide::Bot;
use teloxide::types::Message;

use processor_bili;
use processor_nga::{self, NGAParsed};

mod bot;
mod tests;

static LINK_PROCESSORS: OnceLock<Vec<LinkProcessor>> = OnceLock::new();

#[derive(Debug)]
pub enum BotResponse {
    Text(String),
    Photo(NGAParsed),
    Error(String),
}

#[derive(Debug)]
#[allow(dead_code)]
struct ProcessResult {
    original: String,
    processed: Option<BotResponse>,
}

type ProcessResultAsync = Pin<Box<dyn Future<Output = ProcessResult> + Send>>;

enum ProcessFn {
    // &[&str] 为所有捕获组的字符串切片
    Sync(fn(&[&str]) -> ProcessResult),
    Async(fn(&[&str]) -> ProcessResultAsync),
}

struct LinkProcessor {
    regex: Regex,
    process_fn: ProcessFn,
}

impl LinkProcessor {
    fn new_sync(pattern: &str, process_fn: fn(&[&str]) -> ProcessResult) -> Self {
        Self {
            regex: Regex::new(pattern).unwrap(),
            process_fn: ProcessFn::Sync(process_fn),
        }
    }

    fn new_async(pattern: &str, process_fn: fn(&[&str]) -> ProcessResultAsync) -> Self {
        Self {
            regex: Regex::new(pattern).unwrap(),
            process_fn: ProcessFn::Async(process_fn),
        }
    }
}

// \b 为了区分 fixupx.com 和 x.com
pub const REGEX_X_LINK: &str =
    r"(?:https?://)?\b(?:x\.com|(?:www\.)?twitter\.com)/(\w+)/status/(\d+)";
pub const REGEX_BILI_LINK: &str = r"(?:https?://)?(?:b23\.tv|bili2233\.cn)/([a-zA-Z0-9]+)";
pub const REGEX_NGA_LINK: &str = r"(?:https?://(?:bbs\.nga\.cn|ngabbs\.com|nga\.178\.com|bbs\.gnacn\.cc)[-a-zA-Z0-9@:%_\+.~#?&//=]*)";

fn init_processors() -> Vec<LinkProcessor> {
    vec![
        LinkProcessor::new_sync(REGEX_X_LINK, process_x_link),
        LinkProcessor::new_async(REGEX_BILI_LINK, process_bili_link),
        LinkProcessor::new_async(REGEX_NGA_LINK, process_nga_link),
    ]
}

// 各种链接处理函数
fn process_x_link(captures: &[&str]) -> ProcessResult {
    let link = captures[0]; // 整个匹配的链接
    log::info!("Processing X link: {}", link);

    // 使用捕获组
    // captures[1] 是用户名，captures[2] 是帖子ID
    let processed = if captures.len() >= 3 {
        let username = captures[1];
        let status_id = captures[2];
        log::debug!(
            "X link details - Username: {}, Status ID: {}",
            username,
            status_id
        );
        let processed = format!("https://fxtwitter.com/{}/status/{}", username, status_id);
        Some(BotResponse::Text(processed))
    } else {
        let error = format!("Failed to process X link\n{}", link);
        log::warn!("{}", error);
        Some(BotResponse::Error(error))
    };

    ProcessResult {
        original: link.to_string(),
        processed,
    }
}

fn process_bili_link(captures: &[&str]) -> ProcessResultAsync {
    let link = captures[0].to_string(); // 整个匹配的链接
    log::info!("Processing BiliBili link: {}", link);

    Box::pin(async move {
        let result = processor_bili::get_b23_redirect(&link).await;
        let processed = match result {
            Ok(location) => Some(BotResponse::Text(location)),
            Err(e) => {
                let error = format!("Failed to process BiliBili link\n{}\n{}", link, e);
                log::warn!("{}", error);
                Some(BotResponse::Error(error))
            }
        };

        ProcessResult {
            original: link,
            processed,
        }
    })
}

fn process_nga_link(captures: &[&str]) -> ProcessResultAsync {
    let link = captures[0]; // 整个匹配的链接
    log::info!("Processing NGA link: {}", link);
    let link = link.to_string();
    Box::pin(async move {
        let result = processor_nga::NGAFetcher::parse(&link).await;
        let processed = match result {
            Ok(nga) => Some(BotResponse::Photo(nga)),
            Err(e) => {
                let error = format!("Failed to process NGA link\n{}\n{}", link, e);
                log::warn!("{}", error);
                Some(BotResponse::Error(error))
            }
        };

        ProcessResult {
            original: link,
            processed,
        }
    })
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let bot = Bot::from_env();

    log::info!("Bot started. Listening for messages...");

    teloxide::repl(bot, |bot: Bot, msg: Message| async move {
        if let Some(text) = msg.text() {
            if let Some(responses) = process_links(text).await {
                for resp in responses {
                    match resp {
                        BotResponse::Text(text) => {
                            bot::send_reply_text(&bot, msg.chat.id, msg.id, text).await?;
                        }
                        BotResponse::Photo(nga) => {
                            bot::send_photo(&bot, msg.chat.id, msg.id, nga.urls, nga.text).await?;
                        }
                        BotResponse::Error(err) => {
                            bot::send_reply_text(&bot, msg.chat.id, msg.id, err).await?;
                        }
                    }
                }
            }
        }
        Ok(())
    })
    .await;
}

// 处理链接
async fn process_links(text: &str) -> Option<Vec<BotResponse>> {
    // 快速检查是否包含任何可能的链接特征
    if !text.contains("://") && !text.contains(".com") && !text.contains(".tv") {
        return None;
    }
    // 如果文本过长，只处理前面部分
    const MAX_TEXT_LENGTH: usize = 4000;
    let text = if text.len() > MAX_TEXT_LENGTH {
        &text[..MAX_TEXT_LENGTH]
    } else {
        text
    };

    let processors = LINK_PROCESSORS.get_or_init(init_processors);
    let mut results = Vec::new();

    // 找到所有链接并处理
    for processor in processors {
        for captures in processor.regex.captures_iter(text) {
            // 收集所有捕获组
            let capture_groups: Vec<&str> = captures
                .iter()
                .filter_map(|m| m.map(|m| m.as_str()))
                .collect();

            let result = match &processor.process_fn {
                ProcessFn::Sync(func) => func(&capture_groups),
                ProcessFn::Async(func) => func(&capture_groups).await,
            };

            if let Some(result) = result.processed {
                results.push(result);
            };
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}
