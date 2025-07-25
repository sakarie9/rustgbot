use common::{LinkProcessor, ProcessorResult, ProcessorResultMedia};
use dotenv::dotenv;
use regex::RegexSet;
use std::sync::OnceLock;
use teloxide::Bot;
use teloxide::types::Message;

use processor_bili::BiliBiliProcessor;
use processor_nga::NGALinkProcessor;
use processor_pixiv::PixivLinkProcessor;
use processor_x::XLinkProcessor;

mod bot;
mod tests;

static PROCESSORS: OnceLock<Vec<Box<dyn LinkProcessor>>> = OnceLock::new();
static REGEX_SET: OnceLock<RegexSet> = OnceLock::new();

#[derive(Debug)]
pub enum BotResponse {
    Text(String),
    Photo(ProcessorResultMedia),
    Error(String),
}

fn init_processors() -> Vec<Box<dyn LinkProcessor>> {
    vec![
        Box::new(XLinkProcessor),
        Box::new(BiliBiliProcessor),
        Box::new(NGALinkProcessor),
        Box::new(PixivLinkProcessor),
    ]
}

fn init_regex_set() -> RegexSet {
    let processors = PROCESSORS.get_or_init(init_processors);
    let patterns: Vec<&str> = processors.iter().map(|p| p.pattern()).collect();
    RegexSet::new(&patterns).expect("Failed to create RegexSet")
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
                    let send_result = match resp {
                        BotResponse::Text(text) => {
                            bot::send_reply_text(&bot, msg.chat.id, msg.id, text).await
                        }
                        BotResponse::Photo(media) => {
                            bot::send_photo(&bot, msg.chat.id, msg.id, media.urls, media.caption, media.spoiler)
                                .await
                        }
                        BotResponse::Error(err) => {
                            bot::send_reply_text(&bot, msg.chat.id, msg.id, err).await
                        }
                    };

                    // 记录发送失败的错误，但不中断处理流程
                    if let Err(e) = send_result {
                        log::error!("Failed to send message to chat {}: {}", msg.chat.id, e);
                        if let Err(fallback_err) =
                            bot::send_reply_text(&bot, msg.chat.id, msg.id, e.to_string()).await
                        {
                            log::error!("Failed to send fallback error message: {}", fallback_err);
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
    if !text.contains("://")
        && !text.contains(".com")
        && !text.contains(".tv")
        && !text.contains(".net")
    {
        return None;
    }
    // 如果文本过长，只处理前面部分
    const MAX_TEXT_LENGTH: usize = 4000;
    let text = if text.len() > MAX_TEXT_LENGTH {
        &text[..MAX_TEXT_LENGTH]
    } else {
        text
    };

    let processors = PROCESSORS.get_or_init(init_processors);
    let regex_set = REGEX_SET.get_or_init(init_regex_set);
    let mut results = Vec::new();

    // 使用 RegexSet 快速检查是否有任何匹配
    if !regex_set.is_match(text) {
        return None;
    }

    // 获取所有匹配的模式索引
    let matches: Vec<usize> = regex_set.matches(text).into_iter().collect();

    // 只对匹配的处理器进行详细匹配
    for &match_index in &matches {
        let processor = &processors[match_index];

        // 使用对应的正则表达式进行详细匹配
        for captures in processor.regex().captures_iter(text) {
            log::info!(
                "Processing link with {}: {}",
                processor.name(),
                captures.get(0).unwrap().as_str()
            );

            match processor.process_captures(&captures).await {
                Ok(ProcessorResult::Text(processed_text)) => {
                    results.push(BotResponse::Text(processed_text));
                }
                Ok(ProcessorResult::Media(parsed)) => {
                    results.push(BotResponse::Photo(parsed));
                }
                Err(e) => {
                    let error = format!(
                        "Failed to process link with {}\n{}\n{}",
                        processor.name(),
                        captures.get(0).unwrap().as_str(),
                        e
                    );
                    log::warn!("{}", error);
                    results.push(BotResponse::Error(error));
                }
            }
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}
