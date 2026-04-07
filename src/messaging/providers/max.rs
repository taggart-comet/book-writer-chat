use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;

use crate::core::models::{NormalizedMessage, Provider};

use super::MessengerAdapter;

#[derive(Debug, Default, Clone, Copy)]
pub struct MaxStubAdapter;

#[derive(Debug, Deserialize)]
pub struct MaxWebhook {
    pub room_id: String,
    pub message_id: String,
    pub timestamp: String,
    pub text: String,
    pub sender_display_name: Option<String>,
}

impl MessengerAdapter for MaxStubAdapter {
    type Payload = MaxWebhook;

    fn provider(&self) -> Provider {
        Provider::Max
    }

    fn normalize_payload(
        &self,
        payload: MaxWebhook,
        bot_identity: &str,
    ) -> Result<NormalizedMessage> {
        let timestamp = payload.timestamp.parse::<chrono::DateTime<Utc>>()?;
        Ok(NormalizedMessage {
            provider: self.provider(),
            provider_chat_id: format!("max:{}", payload.room_id),
            message_id: payload.message_id,
            timestamp,
            raw_text: payload.text.clone(),
            attachments: Vec::new(),
            mentions_bot: payload
                .text
                .trim_start()
                .starts_with(&format!("@{bot_identity}")),
            sender_display_name: payload
                .sender_display_name
                .unwrap_or_else(|| "MAX user".to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::providers::normalize_max;

    #[test]
    fn normalizes_max_init_fixture() {
        let normalized = normalize_max(
            serde_json::from_str(include_str!(
                "../../../tests/fixtures/messenger/max-init.json"
            ))
            .unwrap(),
            "bookbot",
        )
        .unwrap();
        assert_eq!(normalized.provider, Provider::Max);
        assert_eq!(normalized.provider_chat_id, "max:room-42");
        assert_eq!(normalized.message_id, "201");
        assert!(normalized.mentions_bot);
        assert_eq!(normalized.sender_display_name, "Bob");
    }

    #[test]
    fn normalizes_max_ignored_fixture() {
        let normalized = normalize_max(
            serde_json::from_str(include_str!(
                "../../../tests/fixtures/messenger/max-ignored.json"
            ))
            .unwrap(),
            "bookbot",
        )
        .unwrap();
        assert!(!normalized.mentions_bot);
    }
}
