use anyhow::Result;
use common::convert_bytes;
use common::extract_filename_from_url;
use common::guess_content_type_from_url;
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
    original_urls: Option<Vec<String>>,
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
            original_urls: None,
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

    /// 设置原始URL列表，用于下载失败时的备用下载 (可选)
    pub fn original_urls(mut self, original_urls: Option<Vec<String>>) -> Self {
        self.original_urls = original_urls;
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
        // 如果只有一个链接，使用统一的媒体发送策略
        send_single_media(msg, bot).await
    } else {
        // 发送媒体组
        Ok(send_photo_group(msg, bot).await?)
    }
}

/// 发送单张媒体文件，根据URL或内容类型智能选择发送方式
/// 如果直接发送URL失败，则下载文件并上传
async fn send_single_media(msg: MessageSenderBuilder, bot: &Bot) -> Result<Message> {
    log::debug!(
        "send_single_media: {}\n\t{}\n\t{}",
        msg.chat_id,
        msg.text,
        msg.urls.join(", ")
    );

    let url = &msg.urls[0];

    // 根据URL扩展名判断媒体类型
    let is_gif = url.ends_with(".gif");

    // 第一次尝试：直接使用URL
    let input_file = InputFile::url(url.parse().unwrap());
    let direct_result = if is_gif {
        bot.send_animation(msg.chat_id, input_file)
            .apply_settings(&msg)
            .await
    } else {
        bot.send_photo(msg.chat_id, input_file)
            .apply_settings(&msg)
            .await
    };

    match direct_result {
        Ok(message) => return Ok(message),
        Err(e) => {
            log::warn!("Direct send failed: {}, trying to download and upload", e);
        }
    }

    // 第二次尝试：下载文件并上传
    // 如果有原始URL且当前URL疑似为Pixiv代理URL，则使用原始URL下载
    let download_url = if let Some(ref original_urls) = msg.original_urls {
        if is_pixiv_related_url(url) && !original_urls.is_empty() {
            &original_urls[0]
        } else {
            url
        }
    } else {
        url
    };

    let data = if is_pixiv_related_url(download_url) {
        log::debug!("Using Pixiv-specific download for: {}", download_url);
        common::download_pixiv(download_url).await
    } else {
        common::download_file(download_url).await
    };

    if let Err(e) = data {
        return Err(anyhow::anyhow!("Failed to download and send media: {}", e));
    }

    let (file_bytes, content_type) = data.unwrap();

    // 记录下载的文件大小
    log::info!(
        "Downloaded single file with size: {} for URL: {}",
        convert_bytes(file_bytes.len() as f64),
        download_url
    );

    // 如果是 application/octet-stream，尝试从URL推断实际的内容类型
    let actual_content_type = match content_type.as_str() {
        "application/octet-stream" => guess_content_type_from_url(url).unwrap_or(content_type),
        _ => content_type,
    };

    // 使用统一的发送函数
    send_file_upload(
        bot,
        msg.chat_id,
        msg.message_id.unwrap_or(MessageId(0)),
        file_bytes,
        &actual_content_type,
        url,
        &msg.text,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to send media: {}", e))
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
            log::info!(
                "Successfully sent media group, total {} files",
                messages.len()
            );
            Ok(messages.remove(0))
        }
        Err(e) => {
            log::warn!(
                "Failed to send media group directly: {}, trying to download and upload",
                e
            );

            // 逐个下载并发送文件
            Ok(send_media_group_with_download(
                bot,
                msg.chat_id,
                msg.message_id.unwrap_or(MessageId(0)),
                msg.urls,
                msg.original_urls,
                msg.text,
                msg.spoiler,
            )
            .await
            .map(|mut messages| messages.remove(0))?)
        }
    }
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
async fn send_media_by_content_type(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    file_bytes: Vec<u8>,
    content_type: &str,
    original_url: &str,
    caption: &str,
) -> ResponseResult<Message> {
    log::debug!(
        "send_media_by_content_type: {}\n\tContent-Type: {}\n\tURL: {}",
        chat_id,
        content_type,
        original_url
    );

    // 根据URL提取文件名，如果无法提取则使用默认名称
    let file_name = extract_filename_from_url(original_url, content_type);
    let input_file = InputFile::memory(file_bytes).file_name(file_name.clone());
    let reply_params = ReplyParameters::new(message_id);

    match content_type {
        // 图片类型
        "image/gif" => {
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

/// 根据文件类型和内容上传文件到Telegram（公共接口）
pub async fn send_file_upload(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    file_bytes: Vec<u8>,
    content_type: &str,
    original_url: &str,
    caption: &str,
) -> ResponseResult<Message> {
    let size = file_bytes.len();
    let file_name = extract_filename_from_url(original_url, content_type);

    log::info!(
        "Downloading and sending file {} with size: {}",
        file_name,
        convert_bytes(size as f64)
    );

    send_media_by_content_type(
        bot,
        chat_id,
        message_id,
        file_bytes,
        content_type,
        original_url,
        caption,
    )
    .await
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
    original_urls: Option<Vec<String>>,
    caption: String,
    spoiler: bool,
) -> ResponseResult<Vec<Message>> {
    let mut downloaded_files = Vec::new();

    // 先下载所有文件
    for (index, url) in media_urls.iter().enumerate() {
        log::debug!(
            "Downloading {}/{} file: {}",
            index + 1,
            media_urls.len(),
            url
        );

        // 确定要下载的URL
        let download_url = if let Some(ref orig_urls) = original_urls {
            if is_pixiv_related_url(url) && index < orig_urls.len() {
                &orig_urls[index]
            } else {
                url
            }
        } else {
            url
        };

        let download_result = if is_pixiv_related_url(download_url) {
            log::debug!("Using Pixiv-specific download for: {}", download_url);
            common::download_pixiv(download_url).await
        } else {
            common::download_file(download_url).await
        };

        match download_result {
            Ok((file_bytes, content_type)) => {
                log::debug!(
                    "Successfully downloaded file {}: {} bytes, content-type: {}",
                    index + 1,
                    file_bytes.len(),
                    content_type
                );

                // 提取文件名
                let file_name = extract_filename_from_url(url, &content_type);
                downloaded_files.push((file_bytes, content_type, file_name, url.clone()));
            }
            Err(_e) => {
                // 存在失败不直接结束，跳过
                log::warn!("Failed to download media file: {url}");
                // return Err(RequestError::Api(ApiError::Unknown(
                //     "Download media group failed".to_string(),
                // )));
            }
        }
    }

    // 计算总文件大小并记录日志
    let total_size: usize = downloaded_files
        .iter()
        .map(|(bytes, _, _, _)| bytes.len())
        .sum();
    log::info!(
        "Downloaded {} files with total size: {}",
        downloaded_files.len(),
        convert_bytes(total_size as f64)
    );

    let caption = if downloaded_files.len() != media_urls.len() {
        // 如果下载的文件数量和URL数量不一致，添加警告信息到caption
        log::warn!(
            "Not all media files were downloaded successfully: {}/{}",
            downloaded_files.len(),
            media_urls.len()
        );
        caption
            + format!(
                "\n[{}/{} Media Downloaded]",
                downloaded_files.len(),
                media_urls.len()
            )
            .as_str()
    } else {
        caption
    };

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
    if let Some(first_media) = media_group.first_mut()
        && let InputMedia::Photo(photo) = first_media
    {
        photo.caption = Some(caption);
        photo.parse_mode = Some(ParseMode::Html);
    }

    // 发送媒体组
    log::info!("Sending media group with {} files", media_count);
    bot.send_media_group(chat_id, media_group)
        .reply_parameters(ReplyParameters::new(message_id))
        .await
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

/// 判断URL是否为Pixiv相关URL（包括代理URL和原始URL）
fn is_pixiv_related_url(url: &str) -> bool {
    const PIXIV_DOMAINS: &[&str] = &[
        "pixiv.net",
        "pximg.net",
        processor_pixiv::constants::REVERSE_PROXY_URL,
    ];

    let from_env = common::get_env_var("PIXIV_IMAGE_PROXY");
    let env_domain_check = from_env
        .as_ref()
        .is_some_and(|domain| !domain.is_empty() && url.contains(domain));

    PIXIV_DOMAINS.iter().any(|domain| url.contains(domain)) || env_domain_check
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::types::{ChatId, MessageId};

    // Mock bot for testing
    struct MockBot;

    impl MockBot {
        fn bot() -> Bot {
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
        let bot = MockBot::bot();
        let chat_id = MockBot::get_chat_id();
        let message_id = MessageId(123);
        let photo_urls = vec![];
        let text = "没有图片的消息".to_string();
        let spoiler = false;

        let result = MessageSenderBuilder::new(chat_id, text)
            .message_id(message_id)
            .urls(photo_urls)
            .spoiler(spoiler)
            .send_photo(&bot)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "需要真实bot token和chat_id，仅手动测试"]
    async fn test_send_photo_single_url() {
        let bot = MockBot::bot();
        let chat_id = MockBot::get_chat_id();
        let message_id = MessageId(123);
        let photo_urls = vec![MockBot::get_photo_url()];
        let text = "单张图片消息".to_string();
        let spoiler = false;

        let result = MessageSenderBuilder::new(chat_id, text)
            .message_id(message_id)
            .urls(photo_urls)
            .spoiler(spoiler)
            .send_photo(&bot)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "需要真实bot token和chat_id，仅手动测试"]
    async fn test_send_photo_multiple_urls() {
        let bot = MockBot::bot();
        let chat_id = MockBot::get_chat_id();
        let message_id = MessageId(123);
        let photo_urls = MockBot::get_photos_url();
        let text = "多张图片消息".to_string();
        let spoiler = false;

        let result = MessageSenderBuilder::new(chat_id, text)
            .message_id(message_id)
            .urls(photo_urls)
            .spoiler(spoiler)
            .send_photo(&bot)
            .await;

        // 应该调用 send_media_group，预期失败但不panic
        assert!(result.is_ok());
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
