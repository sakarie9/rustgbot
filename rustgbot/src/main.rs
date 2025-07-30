use common::{LinkProcessor, ProcessorResult, ProcessorResultMedia};
use dotenv::dotenv;
use regex::RegexSet;
use std::sync::OnceLock;
use teloxide::dispatching::dialogue::GetChatId;
use teloxide::prelude::*;
use teloxide::types::{Message, Update};
use teloxide::{Bot, dptree};

use processor_bili::BiliBiliProcessor;
use processor_nga::NGALinkProcessor;
use processor_pixiv::PixivLinkProcessor;
use processor_x::XLinkProcessor;

use crate::bot::MessageSenderBuilder;

mod bot;
mod commands;
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

    let handler = Update::filter_message()
        .branch(
            // 命令
            dptree::entry()
                .filter_command::<commands::BotCommand>()
                .endpoint(commands::bot_command_handler),
        )
        .branch(
            // 文本
            dptree::filter(|msg: Message| msg.text().is_some()).endpoint(
                |bot: Bot, msg: Message| async move {
                    log::trace!("Received message: {:?}", &msg);
                    process_text_message(&bot, msg).await;
                    Ok(())
                },
            ),
        )
        .branch(
            // 处理私聊GIF消息
            dptree::filter(|msg: Message| msg.chat.is_private()).endpoint(
                |bot: Bot, msg: Message| async move {
                    log::trace!("Received private message: {:?}", &msg);
                    process_private_message(&bot, msg).await;
                    Ok(())
                },
            ),
        );

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn process_text_message(bot: &Bot, msg: Message) {
    let text = msg.text().unwrap();
    let chat_id = msg.chat_id().unwrap();

    if should_skip_message(&msg) {
        log::debug!("Skipping message due to link preview options: {:?}", &msg);
        return;
    }

    if let Some(responses) = process_links(text).await {
        for resp in responses {
            let send_result = match resp {
                BotResponse::Text(text) => {
                    MessageSenderBuilder::new(chat_id, text)
                        .message_id(msg.id)
                        .send_message(bot)
                        .await
                }
                BotResponse::Photo(media) => {
                    MessageSenderBuilder::new(chat_id, media.caption)
                        .message_id(msg.id)
                        .urls(media.urls)
                        .spoiler(media.spoiler)
                        .send_photo(bot)
                        .await
                }
                BotResponse::Error(err) => {
                    MessageSenderBuilder::new(chat_id, err)
                        .message_id(msg.id)
                        .send_message(bot)
                        .await
                }
            };

            // 记录发送失败的错误，但不中断处理流程
            if let Err(e) = send_result {
                log::error!("Failed to send message to chat {}: {}", chat_id, e);
                if let Err(fallback_err) = MessageSenderBuilder::new(chat_id, e.to_string())
                    .message_id(msg.id)
                    .send_message(bot)
                    .await
                {
                    log::error!("Failed to send fallback error message: {}", fallback_err);
                }
            }
        }
    }
}

/// 检查link_preview_options是否存在已经被转换的链接
fn should_skip_message(msg: &Message) -> bool {
    if msg.link_preview_options().is_none() {
        return false;
    }
    if let Some(preview) = msg.link_preview_options() {
        // 链接存在 fixupx.com 或 fxtwitter.com 跳过
        if preview.url.as_deref().map_or(false, |url| {
            url.contains("fixupx.com") || url.contains("fxtwitter.com")
        }) {
            return true;
        }
    }
    false
}

async fn process_private_message(bot: &Bot, msg: Message) {
    // 处理私聊消息
    // 清理 gif caption
    if msg.caption().is_none() {
        return;
    }
    if let Some(animation) = msg.animation() {
        if animation.mime_type != Some("video/mp4".parse().unwrap()) {
            return;
        }
        // 处理动画消息（如GIF）
        let gif_id = animation.file.id.clone();
        if let Err(e) = bot::send_gif_from_fileid(&bot, msg.chat.id, gif_id).await {
            log::error!("Failed to send GIF: {}", e);
        }
    }
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
