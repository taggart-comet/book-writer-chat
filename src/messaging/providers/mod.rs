use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

use crate::core::models::{NormalizedMessage, Provider};

pub mod max;
pub mod telegram;

use self::{max::MaxStubAdapter, telegram::TelegramAdapter};

pub trait MessengerAdapter {
    type Payload: for<'de> Deserialize<'de>;

    fn provider(&self) -> Provider;
    fn normalize_payload(
        &self,
        payload: Self::Payload,
        bot_identity: &str,
    ) -> Result<NormalizedMessage>;

    fn normalize_value(&self, payload: Value, bot_identity: &str) -> Result<NormalizedMessage> {
        let payload = serde_json::from_value(payload)?;
        self.normalize_payload(payload, bot_identity)
    }
}

pub fn normalize_telegram(payload: Value, bot_username: &str) -> Result<NormalizedMessage> {
    TelegramAdapter.normalize_value(payload, bot_username)
}

pub fn normalize_max(payload: Value, bot_handle: &str) -> Result<NormalizedMessage> {
    MaxStubAdapter.normalize_value(payload, bot_handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapters_expose_expected_providers() {
        assert_eq!(TelegramAdapter.provider(), Provider::Telegram);
        assert_eq!(MaxStubAdapter.provider(), Provider::Max);
    }
}
