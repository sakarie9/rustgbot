use teloxide::{prelude::*, utils::command::BotCommands};
use url::Url;

use crate::bot;
use crate::{process_links_full, send_bot_responses};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum BotCommand {
    /// Download a media with given URL.
    Download(String),
    /// Process links in full text without truncation.
    Full(String),
}

pub async fn bot_command_handler(bot: Bot, msg: Message, cmd: BotCommand) -> ResponseResult<()> {
    match cmd {
        BotCommand::Download(url) => {
            let url = match Url::parse(&url) {
                Ok(url) => url,
                Err(_) => {
                    bot::send_reply_text(&bot, msg.chat.id, msg.id, "无效的URL格式。".to_string())
                        .await?;
                    return Ok(());
                }
            };

            // 下载文件
            match common::download_file(url.as_str()).await {
                Ok((file_bytes, content_type)) => {
                    log::info!(
                        "Successfully downloaded file: {} bytes, content-type: {}",
                        file_bytes.len(),
                        content_type
                    );

                    // 上传到Telegram
                    match bot::send_file_upload(
                        &bot,
                        msg.chat.id,
                        msg.id,
                        file_bytes,
                        &content_type,
                        url.as_str(),
                        "",
                    )
                    .await
                    {
                        Ok(_) => {
                            log::info!("Successfully uploaded file to Telegram");
                        }
                        Err(e) => {
                            log::error!("Failed to upload file to Telegram: {}", e);
                            bot::send_reply_text(
                                &bot,
                                msg.chat.id,
                                msg.id,
                                format!("上传文件到Telegram时出错: {}", e),
                            )
                            .await?;
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to download file from {}: {}", url, e);
                    bot::send_reply_text(&bot, msg.chat.id, msg.id, format!("下载文件失败: {}", e))
                        .await?;
                }
            }
        }
        BotCommand::Full(text) => {
            let chat_id = msg.chat.id;

            if let Some(responses) = process_links_full(&text).await {
                send_bot_responses(&bot, chat_id, msg.id, responses).await;
            } else {
                bot::send_reply_text(
                    &bot,
                    msg.chat.id,
                    msg.id,
                    "未在文本中找到支持的链接。".to_string(),
                )
                .await?;
            }
        }
    };

    Ok(())
}
