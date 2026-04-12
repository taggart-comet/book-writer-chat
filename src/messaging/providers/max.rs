use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Deserializer};

use crate::core::models::{MessageAttachment, MessageAttachmentKind, NormalizedMessage, Provider};

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
    #[serde(default)]
    pub attachments: Vec<MessageAttachment>,
}

#[derive(Debug, Deserialize)]
pub struct MaxOfficialUpdate {
    update_type: String,
    message: MaxOfficialMessage,
}

#[derive(Debug, Deserialize)]
pub struct MaxOfficialMessage {
    sender: Option<MaxOfficialUser>,
    recipient: MaxOfficialRecipient,
    body: MaxOfficialMessageBody,
    timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct MaxOfficialUser {
    first_name: Option<String>,
    name: Option<String>,
    username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MaxOfficialRecipient {
    chat_id: Option<i64>,
    user_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MaxOfficialMessageBody {
    mid: String,
    text: Option<String>,
    #[serde(default)]
    attachments: Vec<MaxOfficialAttachment>,
}

#[derive(Debug, Deserialize)]
pub struct MaxOfficialAttachment {
    #[serde(rename = "type")]
    attachment_type: String,
    payload: Option<MaxOfficialAttachmentPayload>,
    filename: Option<String>,
    size: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct MaxOfficialAttachmentPayload {
    photo_id: Option<i64>,
    url: Option<String>,
}

#[derive(Debug)]
pub enum MaxPayload {
    Official(MaxOfficialUpdate),
    Legacy(MaxWebhook),
}

impl<'de> Deserialize<'de> for MaxPayload {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("update_type").is_some() && value.get("message").is_some() {
            Ok(Self::Official(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ))
        } else {
            Ok(Self::Legacy(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ))
        }
    }
}

impl MessengerAdapter for MaxStubAdapter {
    type Payload = MaxPayload;

    fn provider(&self) -> Provider {
        Provider::Max
    }

    fn normalize_payload(
        &self,
        payload: MaxPayload,
        bot_identity: &str,
    ) -> Result<NormalizedMessage> {
        match payload {
            MaxPayload::Official(payload) => {
                normalize_official_payload(self.provider(), payload, bot_identity)
            }
            MaxPayload::Legacy(payload) => {
                normalize_legacy_payload(self.provider(), payload, bot_identity)
            }
        }
    }
}

fn normalize_legacy_payload(
    provider: Provider,
    payload: MaxWebhook,
    bot_identity: &str,
) -> Result<NormalizedMessage> {
    let timestamp = payload.timestamp.parse::<chrono::DateTime<Utc>>()?;
    Ok(NormalizedMessage {
        provider,
        provider_chat_id: format!("max:{}", payload.room_id),
        message_id: payload.message_id,
        timestamp,
        raw_text: payload.text.clone(),
        attachments: payload.attachments,
        mentions_bot: payload
            .text
            .trim_start()
            .starts_with(&format!("@{bot_identity}")),
        sender_display_name: payload
            .sender_display_name
            .unwrap_or_else(|| "MAX user".to_string()),
    })
}

fn normalize_official_payload(
    provider: Provider,
    payload: MaxOfficialUpdate,
    bot_identity: &str,
) -> Result<NormalizedMessage> {
    if payload.update_type != "message_created" {
        return Err(anyhow!(
            "unsupported MAX update type: {}",
            payload.update_type
        ));
    }
    let text = payload.message.body.text.unwrap_or_default();
    let provider_chat_id = payload
        .message
        .recipient
        .chat_id
        .map(|chat_id| format!("max:{chat_id}"))
        .or_else(|| {
            payload
                .message
                .recipient
                .user_id
                .map(|user_id| format!("max:user:{user_id}"))
        })
        .ok_or_else(|| anyhow!("MAX message recipient did not include chat_id or user_id"))?;
    let caption = text.trim().to_string();
    let caption = (!caption.is_empty()).then_some(caption);
    let attachments = max_attachments(payload.message.body.attachments, caption.as_deref())?;
    Ok(NormalizedMessage {
        provider,
        provider_chat_id,
        message_id: payload.message.body.mid,
        timestamp: chrono::DateTime::<Utc>::from_timestamp_millis(payload.message.timestamp)
            .ok_or_else(|| anyhow!("invalid MAX timestamp"))?,
        raw_text: text.clone(),
        attachments,
        mentions_bot: text.trim_start().starts_with(&format!("@{bot_identity}")),
        sender_display_name: payload
            .message
            .sender
            .and_then(|user| user.first_name.or(user.name).or(user.username))
            .unwrap_or_else(|| "MAX user".to_string()),
    })
}

fn max_attachments(
    attachments: Vec<MaxOfficialAttachment>,
    caption: Option<&str>,
) -> Result<Vec<MessageAttachment>> {
    let mut normalized = Vec::new();
    for attachment in attachments {
        if attachment.attachment_type != "image" {
            return Err(anyhow!(
                "unsupported MAX attachment type: {}",
                attachment.attachment_type
            ));
        }
        let payload = attachment
            .payload
            .ok_or_else(|| anyhow!("MAX image attachment did not include payload"))?;
        let url = payload
            .url
            .ok_or_else(|| anyhow!("MAX image attachment did not include url"))?;
        normalized.push(MessageAttachment {
            kind: MessageAttachmentKind::Image,
            provider_file_id: url,
            provider_unique_id: payload.photo_id.map(|photo_id| photo_id.to_string()),
            original_filename: attachment.filename,
            mime_type: Some("image/jpeg".to_string()),
            width: attachment.width,
            height: attachment.height,
            file_size: attachment.size,
            caption: caption.map(ToOwned::to_owned),
        });
    }
    Ok(normalized)
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

    #[test]
    fn normalizes_max_legacy_attachment_fixture_shape() {
        let normalized = normalize_max(
            serde_json::json!({
                "room_id": "room-42",
                "message_id": "204",
                "timestamp": "2026-04-05T10:10:00Z",
                "text": "@bookbot add this chart",
                "sender_display_name": "Bob",
                "attachments": [{
                    "kind": "image",
                    "provider_file_id": "max-file-1",
                    "provider_unique_id": "max-unique-1",
                    "original_filename": "chart.png",
                    "mime_type": "image/png",
                    "width": 640,
                    "height": 480,
                    "file_size": 12345,
                    "caption": "A chart"
                }]
            }),
            "bookbot",
        )
        .unwrap();

        assert_eq!(normalized.attachments.len(), 1);
        assert_eq!(normalized.attachments[0].provider_file_id, "max-file-1");
        assert_eq!(
            normalized.attachments[0].original_filename.as_deref(),
            Some("chart.png")
        );
    }

    #[test]
    fn normalizes_max_official_image_attachment() {
        let normalized = normalize_max(
            serde_json::json!({
                "update_type": "message_created",
                "message": {
                    "sender": {
                        "user_id": 7,
                        "first_name": "Bob",
                        "last_name": null,
                        "username": "bob",
                        "is_bot": false,
                        "last_activity_time": 1775385600000i64
                    },
                    "recipient": {
                        "chat_id": 42,
                        "chat_type": "chat",
                        "user_id": null
                    },
                    "timestamp": 1775385600000i64,
                    "body": {
                        "mid": "204",
                        "seq": 4,
                        "text": "@bookbot add this chart",
                        "attachments": [{
                            "type": "image",
                            "payload": {
                                "photo_id": 12345,
                                "token": "reuse-token",
                                "url": "https://cdn.max.ru/photos/chart.jpg"
                            }
                        }]
                    }
                }
            }),
            "bookbot",
        )
        .unwrap();

        assert_eq!(normalized.provider, Provider::Max);
        assert_eq!(normalized.provider_chat_id, "max:42");
        assert_eq!(normalized.message_id, "204");
        assert!(normalized.mentions_bot);
        assert_eq!(normalized.sender_display_name, "Bob");
        assert_eq!(normalized.attachments.len(), 1);
        let attachment = &normalized.attachments[0];
        assert_eq!(
            attachment.provider_file_id,
            "https://cdn.max.ru/photos/chart.jpg"
        );
        assert_eq!(attachment.provider_unique_id.as_deref(), Some("12345"));
        assert_eq!(attachment.mime_type.as_deref(), Some("image/jpeg"));
    }

    #[test]
    fn rejects_max_unsupported_media_attachment() {
        let result = normalize_max(
            serde_json::json!({
                "update_type": "message_created",
                "message": {
                    "sender": null,
                    "recipient": {"chat_id": 42, "chat_type": "chat", "user_id": null},
                    "timestamp": 1775385600000i64,
                    "body": {
                        "mid": "205",
                        "seq": 5,
                        "text": "@bookbot add this clip",
                        "attachments": [{
                            "type": "video",
                            "payload": {
                                "token": "video-token",
                                "url": "https://cdn.max.ru/video/clip.mp4"
                            }
                        }]
                    }
                }
            }),
            "bookbot",
        );

        assert!(result.is_err());
    }
}
