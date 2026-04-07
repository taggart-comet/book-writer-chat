use anyhow::{Result, anyhow};
use chrono::{TimeZone, Utc};
use serde::Deserialize;

use crate::core::models::{NormalizedMessage, Provider};

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
        let text = payload.message.text.clone().unwrap_or_default();
        Ok(NormalizedMessage {
            provider: self.provider(),
            provider_chat_id: format!("telegram:{}", payload.message.chat.id),
            message_id: payload.message.message_id.to_string(),
            timestamp: Utc
                .timestamp_opt(payload.message.date, 0)
                .single()
                .ok_or_else(|| anyhow!("invalid telegram timestamp"))?,
            raw_text: text.clone(),
            attachments: Vec::new(),
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
    let trimmed = message.text.as_deref().unwrap_or_default().trim_start();
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
}
