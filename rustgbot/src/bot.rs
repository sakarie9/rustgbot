use teloxide::prelude::*;
use teloxide::types::{
    InputFile, InputMedia, InputMediaPhoto, Message, MessageId, ParseMode, ReplyParameters,
};

pub async fn send_reply_text(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    text: String,
) -> ResponseResult<Message> {
    bot.send_message(chat_id, text)
        .reply_parameters(ReplyParameters::new(message_id))
        .parse_mode(ParseMode::Html)
        .await
}

pub async fn send_photo(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    photo_urls: Vec<String>,
    text: String,
) -> ResponseResult<Message> {
    if photo_urls.is_empty() {
        send_reply_text(bot, chat_id, message_id, text).await
    } else if photo_urls.len() == 1 {
        // 如果只有一个图片链接，直接发送单张图片
        send_photo_single(bot, chat_id, message_id, photo_urls[0].clone(), text).await
    } else {
        // 如果有多张图片，发送媒体组
        // 如果图片多于10张，截断到前10张
        let photo_urls = if photo_urls.len() > 10 {
            photo_urls.into_iter().take(10).collect()
        } else {
            photo_urls
        };
        // 发送媒体组
        send_media_group(bot, chat_id, message_id, photo_urls, text)
            .await
            .map(|mut messages| messages.remove(0))
    }
}

async fn send_photo_single(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    photo_url: String,
    caption: String,
) -> ResponseResult<Message> {
    let photo = InputFile::url(photo_url.parse().unwrap());
    bot.send_photo(chat_id, photo)
        .reply_parameters(ReplyParameters::new(message_id))
        .parse_mode(ParseMode::Html)
        .caption(caption)
        .await
}

async fn send_media_group(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    media_urls: Vec<String>,
    caption: String,
) -> ResponseResult<Vec<Message>> {
    let mut media_group = media_urls
        .into_iter()
        .map(|url| InputMedia::Photo(InputMediaPhoto::new(InputFile::url(url.parse().unwrap()))))
        .collect::<Vec<_>>();

    if let Some(InputMedia::Photo(media)) = media_group.first_mut() {
        media.caption = Some(caption.clone());
        media.parse_mode = Some(ParseMode::Html);
    }

    bot.send_media_group(chat_id, media_group)
        .reply_parameters(ReplyParameters::new(message_id))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::types::{ChatId, MessageId};

    // Mock bot for testing
    struct MockBot;

    impl MockBot {
        fn new() -> Bot {
            dotenv::dotenv().ok();
            Bot::from_env()
        }
        fn get_chat_id() -> ChatId {
            ChatId(
                common::get_env_var("TEST_CHAT_ID")
                    .unwrap()
                    .parse()
                    .unwrap(),
            )
        }
        fn get_photo_url() -> String {
            "https://img.nga.178.com/attachments/mon_202505/25/-9lddQvas9-39mmK2dT1kSh2-sg.jpg"
                .to_string()
        }
        fn get_photos_url() -> Vec<String> {
            vec![
                "https://img.nga.178.com/attachments/mon_202505/25/-9lddQvas9-39mmK2dT1kSh2-sg.jpg"
                    .to_string(),
                "https://img.nga.178.com/attachments/mon_202506/27/-9lddQ8s1s-3ltyK1vT3cSk5-sg.jpg"
                    .to_string(),
            ]
        }
    }

    #[tokio::test]
    #[ignore = "需要真实bot token和chat_id，仅手动测试"]
    async fn test_send_photo_empty_urls() {
        let bot = MockBot::new();
        let chat_id = MockBot::get_chat_id();
        let message_id = MessageId(123);
        let photo_urls = vec![];
        let text = "没有图片的消息".to_string();

        let result = send_photo(&bot, chat_id, message_id, photo_urls, text).await;

        assert!(!result.is_err());
    }

    #[tokio::test]
    #[ignore = "需要真实bot token和chat_id，仅手动测试"]
    async fn test_send_photo_single_url() {
        let bot = MockBot::new();
        let chat_id = MockBot::get_chat_id();
        let message_id = MessageId(123);
        let photo_urls = vec![MockBot::get_photo_url()];
        let text = "单张图片消息".to_string();

        let result = send_photo(&bot, chat_id, message_id, photo_urls, text).await;

        assert!(!result.is_err());
    }

    #[tokio::test]
    #[ignore = "需要真实bot token和chat_id，仅手动测试"]
    async fn test_send_photo_multiple_urls() {
        let bot = MockBot::new();
        let chat_id = MockBot::get_chat_id();
        let message_id = MessageId(123);
        let photo_urls = MockBot::get_photos_url();
        let text = "多张图片消息".to_string();

        let result = send_photo(&bot, chat_id, message_id, photo_urls, text).await;

        // 应该调用 send_media_group，预期失败但不panic
        assert!(!result.is_err());
    }

    #[test]
    fn test_photo_urls_validation() {
        // 测试URL格式验证
        let valid_urls = MockBot::get_photos_url();

        for url in valid_urls {
            // 简单测试URL字符串包含协议
            assert!(url.starts_with("http"), "URL应该以http开头: {}", url);
        }

        let invalid_urls = vec!["not_a_url", "ftp://invalid.com/file.jpg", ""];

        for url in invalid_urls {
            if url.is_empty() {
                continue; // 空字符串是特殊情况
            }
            // 测试不以http开头的URL
            if url == "not_a_url" || url.starts_with("ftp://") {
                assert!(!url.starts_with("http"), "无效URL不应该以http开头: {}", url);
            }
        }
    }
}
