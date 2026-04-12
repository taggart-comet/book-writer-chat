use anyhow::{Result, anyhow};
use chrono::{TimeZone, Utc};
use serde::Deserialize;

use crate::core::models::{MessageAttachment, MessageAttachmentKind, NormalizedMessage, Provider};

use super::MessengerAdapter;

#[derive(Debug, Default, Clone, Copy)]
pub struct TelegramAdapter;

#[derive(Debug, Deserialize)]
pub struct TelegramWebhook {
    pub message: TelegramMessage,
}

#[derive(Debug, Deserialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub date: i64,
    pub text: Option<String>,
    pub caption: Option<String>,
    pub photo: Option<Vec<TelegramPhotoSize>>,
    pub document: Option<TelegramDocument>,
    pub chat: TelegramChat,
    pub from: Option<TelegramUser>,
    pub reply_to_message: Option<ReplyToMessage>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TelegramUser {
    pub first_name: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReplyToMessage {
    pub from: Option<ReplyUser>,
}

#[derive(Debug, Deserialize)]
pub struct ReplyUser {
    pub username: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TelegramPhotoSize {
    pub file_id: String,
    pub file_unique_id: String,
    pub width: u32,
    pub height: u32,
    pub file_size: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TelegramDocument {
    pub file_id: String,
    pub file_unique_id: String,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<u64>,
}

impl MessengerAdapter for TelegramAdapter {
    type Payload = TelegramWebhook;

    fn provider(&self) -> Provider {
        Provider::Telegram
    }

    fn normalize_payload(
        &self,
        payload: TelegramWebhook,
        bot_identity: &str,
    ) -> Result<NormalizedMessage> {
        let text = payload
            .message
            .text
            .clone()
            .or_else(|| payload.message.caption.clone())
            .unwrap_or_default();
        Ok(NormalizedMessage {
            provider: self.provider(),
            provider_chat_id: format!("telegram:{}", payload.message.chat.id),
            message_id: payload.message.message_id.to_string(),
            timestamp: Utc
                .timestamp_opt(payload.message.date, 0)
                .single()
                .ok_or_else(|| anyhow!("invalid telegram timestamp"))?,
            raw_text: text.clone(),
            attachments: telegram_attachments(&payload.message),
            mentions_bot: telegram_mentions_bot(&payload.message, bot_identity),
            sender_display_name: payload
                .message
                .from
                .clone()
                .and_then(|user| user.first_name)
                .unwrap_or_else(|| "Telegram user".to_string()),
        })
    }
}

pub fn telegram_mentions_bot(message: &TelegramMessage, bot_username: &str) -> bool {
    let trimmed = message
        .text
        .as_deref()
        .or(message.caption.as_deref())
        .unwrap_or_default()
        .trim_start();
    if trimmed.starts_with(&format!("@{bot_username}"))
        || trimmed.starts_with("/bookbot")
        || trimmed.starts_with(&format!("/{bot_username}"))
    {
        return true;
    }

    message
        .reply_to_message
        .as_ref()
        .and_then(|reply| reply.from.as_ref())
        .and_then(|user| user.username.as_deref())
        .is_some_and(|username| username == bot_username)
        && !trimmed.is_empty()
}

fn telegram_attachments(message: &TelegramMessage) -> Vec<MessageAttachment> {
    let caption = message
        .caption
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(photo) = message.photo.as_ref().and_then(|sizes| {
        sizes.iter().max_by_key(|size| {
            (
                size.file_size.unwrap_or_default(),
                u64::from(size.width) * u64::from(size.height),
            )
        })
    }) {
        return vec![MessageAttachment {
            kind: MessageAttachmentKind::Image,
            provider_file_id: photo.file_id.clone(),
            provider_unique_id: Some(photo.file_unique_id.clone()),
            original_filename: None,
            mime_type: Some("image/jpeg".to_string()),
            width: Some(photo.width),
            height: Some(photo.height),
            file_size: photo.file_size,
            caption,
        }];
    }

    message
        .document
        .as_ref()
        .filter(|document| {
            document
                .mime_type
                .as_deref()
                .is_some_and(|mime_type| mime_type.starts_with("image/"))
        })
        .map(|document| {
            vec![MessageAttachment {
                kind: MessageAttachmentKind::Image,
                provider_file_id: document.file_id.clone(),
                provider_unique_id: Some(document.file_unique_id.clone()),
                original_filename: document.file_name.clone(),
                mime_type: document.mime_type.clone(),
                width: None,
                height: None,
                file_size: document.file_size,
                caption,
            }]
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::messaging::providers::normalize_telegram;

    #[test]
    fn normalizes_telegram_init_fixture() {
        let normalized = normalize_telegram(
            serde_json::from_str(include_str!(
                "../../../tests/fixtures/messenger/telegram-init.json"
            ))
            .unwrap(),
            "bookbot",
        )
        .unwrap();
        assert_eq!(normalized.provider, Provider::Telegram);
        assert_eq!(normalized.provider_chat_id, "telegram:123456");
        assert_eq!(normalized.message_id, "101");
        assert_eq!(
            normalized.timestamp,
            Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap()
        );
        assert_eq!(normalized.raw_text, "/bookbot init");
        assert!(normalized.mentions_bot);
        assert_eq!(normalized.sender_display_name, "Alice");
    }

    #[test]
    fn normalizes_telegram_reply_authoring_fixture() {
        let normalized = normalize_telegram(
            serde_json::from_str(include_str!(
                "../../../tests/fixtures/messenger/telegram-authoring-reply.json"
            ))
            .unwrap(),
            "bookbot",
        )
        .unwrap();
        assert!(normalized.mentions_bot);
        assert_eq!(
            normalized.raw_text,
            "Write an introductory chapter about habit formation for busy parents."
        );
    }

    #[test]
    fn normalizes_telegram_ignored_fixture() {
        let normalized = normalize_telegram(
            serde_json::from_str(include_str!(
                "../../../tests/fixtures/messenger/telegram-ignored.json"
            ))
            .unwrap(),
            "bookbot",
        )
        .unwrap();
        assert!(!normalized.mentions_bot);
    }

    #[test]
    fn telegram_reply_to_bot_counts_as_bot_directed() {
        let normalized = TelegramAdapter
            .normalize_payload(
                TelegramWebhook {
                    message: TelegramMessage {
                        message_id: 999,
                        date: 1_775_385_600,
                        text: Some("status".to_string()),
                        caption: None,
                        photo: None,
                        document: None,
                        chat: TelegramChat {
                            id: 42,
                            title: Some("Chat".to_string()),
                        },
                        from: Some(TelegramUser {
                            first_name: Some("Alice".to_string()),
                            username: Some("alice".to_string()),
                        }),
                        reply_to_message: Some(ReplyToMessage {
                            from: Some(ReplyUser {
                                username: Some("bookbot".to_string()),
                            }),
                        }),
                    },
                },
                "bookbot",
            )
            .unwrap();
        assert!(normalized.mentions_bot);
    }

    #[test]
    fn telegram_photo_caption_counts_as_bot_directed_and_selects_largest_photo() {
        let normalized = TelegramAdapter
            .normalize_payload(
                TelegramWebhook {
                    message: TelegramMessage {
                        message_id: 1000,
                        date: 1_775_385_600,
                        text: None,
                        caption: Some("@bookbot place this near the lighthouse scene".to_string()),
                        photo: Some(vec![
                            TelegramPhotoSize {
                                file_id: "small-file".to_string(),
                                file_unique_id: "small-unique".to_string(),
                                width: 320,
                                height: 200,
                                file_size: Some(2_000),
                            },
                            TelegramPhotoSize {
                                file_id: "large-file".to_string(),
                                file_unique_id: "large-unique".to_string(),
                                width: 1280,
                                height: 800,
                                file_size: Some(20_000),
                            },
                        ]),
                        document: None,
                        chat: TelegramChat {
                            id: 42,
                            title: Some("Chat".to_string()),
                        },
                        from: Some(TelegramUser {
                            first_name: Some("Alice".to_string()),
                            username: Some("alice".to_string()),
                        }),
                        reply_to_message: None,
                    },
                },
                "bookbot",
            )
            .unwrap();

        assert!(normalized.mentions_bot);
        assert_eq!(
            normalized.raw_text,
            "@bookbot place this near the lighthouse scene"
        );
        assert_eq!(normalized.attachments.len(), 1);
        let attachment = &normalized.attachments[0];
        assert_eq!(attachment.provider_file_id, "large-file");
        assert_eq!(
            attachment.provider_unique_id.as_deref(),
            Some("large-unique")
        );
        assert_eq!(attachment.mime_type.as_deref(), Some("image/jpeg"));
        assert_eq!(attachment.width, Some(1280));
        assert_eq!(attachment.height, Some(800));
    }

    #[test]
    fn telegram_image_document_normalizes_as_attachment() {
        let normalized = TelegramAdapter
            .normalize_payload(
                TelegramWebhook {
                    message: TelegramMessage {
                        message_id: 1001,
                        date: 1_775_385_600,
                        text: None,
                        caption: Some("@bookbot add this diagram".to_string()),
                        photo: None,
                        document: Some(TelegramDocument {
                            file_id: "document-file".to_string(),
                            file_unique_id: "document-unique".to_string(),
                            file_name: Some("diagram.png".to_string()),
                            mime_type: Some("image/png".to_string()),
                            file_size: Some(4096),
                        }),
                        chat: TelegramChat {
                            id: 42,
                            title: Some("Chat".to_string()),
                        },
                        from: None,
                        reply_to_message: None,
                    },
                },
                "bookbot",
            )
            .unwrap();

        assert_eq!(normalized.attachments.len(), 1);
        let attachment = &normalized.attachments[0];
        assert_eq!(attachment.provider_file_id, "document-file");
        assert_eq!(attachment.original_filename.as_deref(), Some("diagram.png"));
        assert_eq!(attachment.mime_type.as_deref(), Some("image/png"));
    }

    #[test]
    fn telegram_non_image_document_is_not_an_attachment() {
        let normalized = TelegramAdapter
            .normalize_payload(
                TelegramWebhook {
                    message: TelegramMessage {
                        message_id: 1002,
                        date: 1_775_385_600,
                        text: None,
                        caption: Some("@bookbot read this".to_string()),
                        photo: None,
                        document: Some(TelegramDocument {
                            file_id: "document-file".to_string(),
                            file_unique_id: "document-unique".to_string(),
                            file_name: Some("notes.pdf".to_string()),
                            mime_type: Some("application/pdf".to_string()),
                            file_size: Some(4096),
                        }),
                        chat: TelegramChat {
                            id: 42,
                            title: Some("Chat".to_string()),
                        },
                        from: None,
                        reply_to_message: None,
                    },
                },
                "bookbot",
            )
            .unwrap();

        assert!(normalized.attachments.is_empty());
    }
}
