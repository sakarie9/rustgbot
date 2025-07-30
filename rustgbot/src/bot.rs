use anyhow::Result;
use teloxide::ApiError;
use teloxide::RequestError;
use teloxide::payloads::SendAnimation;
use teloxide::payloads::SendPhoto;
use teloxide::prelude::*;
use teloxide::requests::MultipartRequest;
use teloxide::types::FileId;
use teloxide::types::{
    InputFile, InputMedia, InputMediaPhoto, Message, MessageId, ParseMode, ReplyParameters,
};

/// 通用的请求配置 trait
trait ApplyMessageSettings<T> {
    fn apply_settings(self, msg: &MessageSenderBuilder) -> T;
}

impl ApplyMessageSettings<MultipartRequest<SendPhoto>> for MultipartRequest<SendPhoto> {
    fn apply_settings(mut self, msg: &MessageSenderBuilder) -> MultipartRequest<SendPhoto> {
        self = self.parse_mode(ParseMode::Html).caption(msg.text.clone());

        if let Some(message_id) = msg.message_id {
            self = self.reply_parameters(ReplyParameters::new(message_id));
        }

        if msg.spoiler {
            self = self.has_spoiler(true);
        }

        self
    }
}

impl ApplyMessageSettings<MultipartRequest<SendAnimation>> for MultipartRequest<SendAnimation> {
    fn apply_settings(mut self, msg: &MessageSenderBuilder) -> MultipartRequest<SendAnimation> {
        self = self.parse_mode(ParseMode::Html).caption(msg.text.clone());

        if let Some(message_id) = msg.message_id {
            self = self.reply_parameters(ReplyParameters::new(message_id));
        }

        self
    }
}

#[derive(Clone)]
pub struct MessageSenderBuilder {
    chat_id: ChatId,
    message_id: Option<MessageId>,
    text: String,
    urls: Vec<String>,
    spoiler: bool,
}

impl MessageSenderBuilder {
    /// 创建一个新的建造者实例。
    /// bot, chat_id, 和 text 是必需的。
    pub fn new(chat_id: ChatId, text: String) -> Self {
        Self {
            chat_id,
            text,
            // 以下是可选参数的默认值
            message_id: None,
            urls: Vec::new(),
            spoiler: false,
        }
    }

    /// 设置要回复的消息 ID (可选)
    pub fn message_id(mut self, message_id: MessageId) -> Self {
        self.message_id = Some(message_id);
        self
    }

    /// 设置媒体链接 (可选)
    pub fn urls(mut self, urls: Vec<String>) -> Self {
        // 如果图片多于10张，截断到前10张
        let photo_urls = if urls.len() > 10 {
            urls.into_iter().take(10).collect()
        } else {
            urls
        };

        self.urls = photo_urls;
        self
    }

    /// 设置是否剧透 (可选)
    pub fn spoiler(mut self, spoiler: bool) -> Self {
        self.spoiler = spoiler;
        self
    }

    pub async fn send_message(self, bot: &Bot) -> Result<Message> {
        send_message(self, bot).await
    }

    pub async fn send_photo(self, bot: &Bot) -> Result<Message> {
        send_photo(self, bot).await
    }
}

/// 封装
async fn send_message(msg: MessageSenderBuilder, bot: &Bot) -> Result<Message> {
    log::debug!("send_reply_text: {}\n\t{}", msg.chat_id, msg.text);
    let mut request = bot
        .send_message(msg.chat_id, msg.text)
        .parse_mode(ParseMode::Html);

    if let Some(message_id) = msg.message_id {
        request = request.reply_parameters(ReplyParameters::new(message_id));
    }

    Ok(request.await?)
}

/// 发送图片
/// 自动处理单张图片和多张图片的情况
async fn send_photo(msg: MessageSenderBuilder, bot: &Bot) -> Result<Message> {
    if msg.urls.is_empty() {
        send_message(msg, bot).await
    } else if msg.urls.len() == 1 {
        // 如果只有一个链接，使用智能下载策略
        // 如果是 gif，则使用 send_gif，否则使用 send_photo_single
        if msg.urls[0].ends_with(".gif") {
            send_gif(msg, bot).await
        } else {
            send_photo_single(msg, bot).await
        }
    } else {
        // 发送媒体组
        Ok(send_photo_group(msg, bot).await?)
    }
}

/// 发送单张图片，如果失败则尝试下载并上传
async fn send_photo_single(msg: MessageSenderBuilder, bot: &Bot) -> Result<Message> {
    log::debug!(
        "send_photo_single: {}\n\t{}\n\t{}",
        msg.chat_id,
        msg.text,
        msg.urls.join(", ")
    );

    // 第一次尝试：直接使用URL
    let photo = InputFile::url(msg.urls[0].parse().unwrap());
    let request = bot.send_photo(msg.chat_id, photo).apply_settings(&msg);

    match request.await {
        Ok(message) => return Ok(message),
        Err(e) => {
            log::warn!("直接发送URL失败: {}, 尝试下载并上传", e);
        }
    }

    // 第二次尝试：下载文件并上传
    let data = common::download_file(&msg.urls[0]).await;
    if let Err(e) = data {
        return Err(anyhow::anyhow!("Failed to download and send photo: {}", e));
    }

    let (file_bytes, _content_type) = data.unwrap();
    let photo = InputFile::memory(file_bytes).file_name("image.png");
    let request = bot.send_photo(msg.chat_id, photo).apply_settings(&msg);

    Ok(request.await?)
}

/// 发送多张图片，如果失败则尝试下载并上传
async fn send_photo_group(msg: MessageSenderBuilder, bot: &Bot) -> Result<Message> {
    log::debug!(
        "send_media_group: {}\n{}\n{}",
        msg.chat_id,
        msg.text,
        msg.urls.join(", ")
    );

    // 先尝试直接发送URL媒体组
    let direct_result = send_media_group_direct(
        bot,
        msg.chat_id,
        msg.message_id.unwrap_or(MessageId(0)),
        &msg.urls,
        &msg.text,
        msg.spoiler,
    )
    .await;

    match direct_result {
        Ok(mut messages) => {
            log::info!("成功直接发送媒体组，共 {} 个文件", messages.len());
            Ok(messages.remove(0))
        }
        Err(e) => {
            log::warn!("直接发送媒体组失败: {}，尝试逐个下载上传", e);

            // 逐个下载并发送文件
            Ok(send_media_group_with_download(
                bot,
                msg.chat_id,
                msg.message_id.unwrap_or(MessageId(0)),
                msg.urls,
                msg.text,
                msg.spoiler,
            )
            .await
            .map(|mut messages| messages.remove(0))?)
        }
    }
}

/// 发送单张GIF，如果失败则尝试下载并上传
pub async fn send_gif(msg: MessageSenderBuilder, bot: &Bot) -> Result<Message> {
    log::debug!(
        "send_gif: {}\n\t{}\n\t{}",
        msg.chat_id,
        msg.text,
        msg.urls.join(", ")
    );

    // 第一次尝试：直接使用URL
    let photo = InputFile::url(msg.urls[0].parse().unwrap());
    let request = bot.send_animation(msg.chat_id, photo).apply_settings(&msg);

    match request.await {
        Ok(message) => return Ok(message),
        Err(e) => {
            log::warn!("直接发送URL失败: {}, 尝试下载并上传", e);
        }
    }

    // 第二次尝试：下载文件并上传
    let data = common::download_file(&msg.urls[0]).await;
    if let Err(e) = data {
        return Err(anyhow::anyhow!("Failed to download and send photo: {}", e));
    }

    let (file_bytes, _content_type) = data.unwrap();
    let photo = InputFile::memory(file_bytes).file_name("image.gif");
    let request = bot.send_animation(msg.chat_id, photo).apply_settings(&msg);

    Ok(request.await?)
}

/// 用file_id发送GIF
pub async fn send_gif_from_fileid(
    bot: &Bot,
    chat_id: ChatId,
    file_id: FileId,
) -> ResponseResult<Message> {
    log::debug!("send_gif_from_fileid: {}\n\t{}", chat_id, file_id);
    bot.send_animation(chat_id, InputFile::file_id(file_id))
        .await
}

/// 根据文件类型和内容上传文件到Telegram
pub async fn send_file_upload(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    file_bytes: Vec<u8>,
    content_type: &str,
    original_url: &str,
    caption: &str,
) -> ResponseResult<Message> {
    log::debug!(
        "send_file_upload: {}\n\tContent-Type: {}\n\tURL: {}",
        chat_id,
        content_type,
        original_url
    );

    // 根据URL提取文件名，如果无法提取则使用默认名称
    let file_name = extract_filename_from_url(original_url, content_type);
    let input_file = InputFile::memory(file_bytes).file_name(file_name);
    let reply_params = ReplyParameters::new(message_id);

    match content_type {
        // 图片类型
        ct if ct.starts_with("image/gif") => {
            bot.send_animation(chat_id, input_file)
                .reply_parameters(reply_params)
                .parse_mode(ParseMode::Html)
                .caption(caption)
                .await
        }
        ct if ct.starts_with("image/") => {
            bot.send_photo(chat_id, input_file)
                .reply_parameters(reply_params)
                .parse_mode(ParseMode::Html)
                .caption(caption)
                .await
        }
        // 视频类型
        ct if ct.starts_with("video/") => {
            bot.send_video(chat_id, input_file)
                .reply_parameters(reply_params)
                .parse_mode(ParseMode::Html)
                .caption(caption)
                .await
        }
        // 音频类型
        ct if ct.starts_with("audio/") => {
            bot.send_audio(chat_id, input_file)
                .reply_parameters(reply_params)
                .parse_mode(ParseMode::Html)
                .caption(caption)
                .await
        }
        // 其他文件类型作为文档发送
        _ => {
            bot.send_document(chat_id, input_file)
                .reply_parameters(reply_params)
                .parse_mode(ParseMode::Html)
                .caption(caption)
                .await
        }
    }
}

/// 直接发送URL媒体组
async fn send_media_group_direct(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    media_urls: &[String],
    caption: &str,
    spoiler: bool,
) -> ResponseResult<Vec<Message>> {
    let mut media_group = media_urls
        .iter()
        .map(|url| {
            let mut photo = InputMediaPhoto::new(InputFile::url(url.parse().unwrap()));
            photo.has_spoiler = spoiler;
            InputMedia::Photo(photo)
        })
        .collect::<Vec<_>>();

    if let Some(InputMedia::Photo(media)) = media_group.first_mut() {
        media.caption = Some(caption.to_string());
        media.parse_mode = Some(ParseMode::Html);
    }

    bot.send_media_group(chat_id, media_group)
        .reply_parameters(ReplyParameters::new(message_id))
        .await
}

/// 通过下载上传的方式发送媒体组
async fn send_media_group_with_download(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    media_urls: Vec<String>,
    caption: String,
    spoiler: bool,
) -> ResponseResult<Vec<Message>> {
    let mut downloaded_files = Vec::new();

    // 先下载所有文件
    for (index, url) in media_urls.iter().enumerate() {
        log::info!("下载第 {}/{} 个文件: {}", index + 1, media_urls.len(), url);

        match common::download_file(url).await {
            Ok((file_bytes, content_type)) => {
                log::info!(
                    "成功下载第 {} 个文件: {} bytes, content-type: {}",
                    index + 1,
                    file_bytes.len(),
                    content_type
                );

                // 提取文件名
                let file_name = extract_filename_from_url(url, &content_type);
                downloaded_files.push((file_bytes, content_type, file_name, url.clone()));
            }
            Err(_e) => {
                // 存在失败直接结束
                // let _ = send_reply_text(bot, chat_id, message_id, "下载失败".to_string()).await;
                return Err(RequestError::Api(ApiError::Unknown("下载失败".to_string())));
            }
        }
    }

    // 构建媒体组
    let mut media_group = Vec::new();
    for (file_bytes, _content_type, file_name, _url) in downloaded_files {
        let input_file = InputFile::memory(file_bytes).file_name(file_name);

        let mut photo = InputMediaPhoto::new(input_file);
        photo.has_spoiler = spoiler;

        media_group.push(InputMedia::Photo(photo));
    }

    // 为第一个媒体添加caption
    let media_count = media_group.len();
    if let Some(first_media) = media_group.first_mut() {
        match first_media {
            InputMedia::Photo(photo) => {
                photo.caption = Some(caption);
                photo.parse_mode = Some(ParseMode::Html);
            }
            _ => {}
        }
    }

    // 发送媒体组
    log::info!("发送包含 {} 个文件的媒体组", media_count);
    bot.send_media_group(chat_id, media_group)
        .reply_parameters(ReplyParameters::new(message_id))
        .await
}

/// 从URL中提取文件名，如果无法提取则根据content-type生成默认文件名
fn extract_filename_from_url(url: &str, content_type: &str) -> String {
    use std::path::Path;

    // 尝试从URL路径中提取文件名
    if let Ok(parsed_url) = url::Url::parse(url) {
        let path = parsed_url.path();
        if let Some(filename) = Path::new(path).file_name() {
            if let Some(filename_str) = filename.to_str() {
                if !filename_str.is_empty() && filename_str != "/" {
                    return filename_str.to_string();
                }
            }
        }
    }

    // 如果无法从URL提取文件名，根据content-type生成默认文件名
    get_file_extension_from_content_type(content_type)
}

/// 根据content-type获取对应的文件扩展名
fn get_file_extension_from_content_type(content_type: &str) -> String {
    let extension = if content_type.starts_with("image/") {
        match content_type {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => "jpg", // 默认图片格式
        }
    } else if content_type.starts_with("video/") {
        match content_type {
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            "video/avi" => "avi",
            _ => "mp4", // 默认视频格式
        }
    } else if content_type.starts_with("audio/") {
        match content_type {
            "audio/mpeg" => "mp3",
            "audio/wav" => "wav",
            "audio/ogg" => "ogg",
            _ => "mp3", // 默认音频格式
        }
    } else {
        match content_type {
            "application/pdf" => "pdf",
            "application/zip" => "zip",
            "text/plain" => "txt",
            _ => "bin",
        }
    };

    format!("file.{}", extension)
}

// 简单的发送文本回复
pub async fn send_reply_text(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    text: String,
) -> ResponseResult<Message> {
    log::debug!("send_reply_text: {}\n\t{}", chat_id, text);
    bot.send_message(chat_id, text)
        .reply_parameters(ReplyParameters::new(message_id))
        .parse_mode(ParseMode::Html)
        .await
}

// /// 尝试从URL发送动画
// async fn send_animation_from_url(
//     bot: &Bot,
//     chat_id: ChatId,
//     message_id: MessageId,
//     url: &str,
//     caption: &str,
// ) -> ResponseResult<Message> {
//     log::debug!(
//         "send_animation_from_url: {}\n\t{}\n\t{}",
//         chat_id,
//         caption,
//         url
//     );

//     let parsed_url = match url::Url::parse(url) {
//         Ok(url) => url,
//         Err(e) => {
//             return Err(RequestError::Api(ApiError::Unknown(format!(
//                 "Invalid URL: {}",
//                 e
//             ))));
//         }
//     };

//     let input_file = InputFile::url(parsed_url);

//     bot.send_animation(chat_id, input_file)
//         .reply_parameters(ReplyParameters::new(message_id))
//         .parse_mode(ParseMode::Html)
//         .caption(caption)
//         .await
// }

// /// 尝试从URL发送视频
// async fn send_video_from_url(
//     bot: &Bot,
//     chat_id: ChatId,
//     message_id: MessageId,
//     url: &str,
//     caption: &str,
// ) -> ResponseResult<Message> {
//     log::debug!("send_video_from_url: {}\n\t{}\n\t{}", chat_id, caption, url);

//     let parsed_url = match url::Url::parse(url) {
//         Ok(url) => url,
//         Err(e) => {
//             return Err(RequestError::Api(ApiError::Unknown(format!(
//                 "Invalid URL: {}",
//                 e
//             ))));
//         }
//     };

//     let input_file = InputFile::url(parsed_url);

//     bot.send_video(chat_id, input_file)
//         .reply_parameters(ReplyParameters::new(message_id))
//         .parse_mode(ParseMode::Html)
//         .caption(caption)
//         .await
// }

// /// 尝试让Telegram直接从URL下载文件
// pub async fn send_document_from_url(
//     bot: &Bot,
//     chat_id: ChatId,
//     message_id: MessageId,
//     url: &str,
//     caption: &str,
// ) -> ResponseResult<Message> {
//     log::debug!(
//         "send_document_from_url: {}\n\t{}\n\t{}",
//         chat_id,
//         caption,
//         url
//     );

//     // 尝试解析URL
//     let parsed_url = match url::Url::parse(url) {
//         Ok(url) => url,
//         Err(e) => {
//             return Err(RequestError::Api(ApiError::Unknown(format!(
//                 "Invalid URL: {}",
//                 e
//             ))));
//         }
//     };

//     let input_file = InputFile::url(parsed_url);

//     // 先尝试作为文档发送
//     bot.send_document(chat_id, input_file)
//         .reply_parameters(ReplyParameters::new(message_id))
//         .parse_mode(ParseMode::Html)
//         .caption(caption)
//         .await
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use teloxide::types::{ChatId, MessageId};

//     // Mock bot for testing
//     struct MockBot;

//     impl MockBot {
//         fn new() -> Bot {
//             dotenv::dotenv().ok();
//             Bot::from_env()
//         }
//         fn get_chat_id() -> ChatId {
//             ChatId(
//                 common::get_env_var("TEST_CHAT_ID")
//                     .unwrap()
//                     .parse()
//                     .unwrap(),
//             )
//         }
//         fn get_photo_url() -> String {
//             "https://img.nga.178.com/attachments/mon_202505/25/-9lddQvas9-39mmK2dT1kSh2-sg.jpg"
//                 .to_string()
//         }
//         fn get_photos_url() -> Vec<String> {
//             vec![
//                 "https://img.nga.178.com/attachments/mon_202505/25/-9lddQvas9-39mmK2dT1kSh2-sg.jpg"
//                     .to_string(),
//                 "https://img.nga.178.com/attachments/mon_202506/27/-9lddQ8s1s-3ltyK1vT3cSk5-sg.jpg"
//                     .to_string(),
//             ]
//         }
//     }

//     #[tokio::test]
//     #[ignore = "需要真实bot token和chat_id，仅手动测试"]
//     async fn test_send_photo_empty_urls() {
//         let bot = MockBot::new();
//         let chat_id = MockBot::get_chat_id();
//         let message_id = MessageId(123);
//         let photo_urls = vec![];
//         let text = "没有图片的消息".to_string();
//         let spoiler = false;

//         let result = send_photo(&bot, chat_id, message_id, photo_urls, text, spoiler).await;

//         assert!(!result.is_err());
//     }

//     #[tokio::test]
//     #[ignore = "需要真实bot token和chat_id，仅手动测试"]
//     async fn test_send_photo_single_url() {
//         let bot = MockBot::new();
//         let chat_id = MockBot::get_chat_id();
//         let message_id = MessageId(123);
//         let photo_urls = vec![MockBot::get_photo_url()];
//         let text = "单张图片消息".to_string();
//         let spoiler = false;

//         let result = send_photo(&bot, chat_id, message_id, photo_urls, text, spoiler).await;

//         assert!(!result.is_err());
//     }

//     #[tokio::test]
//     #[ignore = "需要真实bot token和chat_id，仅手动测试"]
//     async fn test_send_photo_multiple_urls() {
//         let bot = MockBot::new();
//         let chat_id = MockBot::get_chat_id();
//         let message_id = MessageId(123);
//         let photo_urls = MockBot::get_photos_url();
//         let text = "多张图片消息".to_string();
//         let spoiler = false;

//         let result = send_photo(&bot, chat_id, message_id, photo_urls, text, spoiler).await;

//         // 应该调用 send_media_group，预期失败但不panic
//         assert!(!result.is_err());
//     }

//     #[test]
//     fn test_photo_urls_validation() {
//         // 测试URL格式验证
//         let valid_urls = MockBot::get_photos_url();

//         for url in valid_urls {
//             // 简单测试URL字符串包含协议
//             assert!(url.starts_with("http"), "URL应该以http开头: {}", url);
//         }

//         let invalid_urls = vec!["not_a_url", "ftp://invalid.com/file.jpg", ""];

//         for url in invalid_urls {
//             if url.is_empty() {
//                 continue; // 空字符串是特殊情况
//             }
//             // 测试不以http开头的URL
//             if url == "not_a_url" || url.starts_with("ftp://") {
//                 assert!(!url.starts_with("http"), "无效URL不应该以http开头: {}", url);
//             }
//         }
//     }
// }
