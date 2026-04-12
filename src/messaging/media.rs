use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use reqwest::Url;
use serde::Deserialize;

use crate::{
    core::{
        config::Config,
        models::{MessageAttachment, Provider},
    },
    storage::media_assets::DownloadedMedia,
};

#[async_trait]
pub trait MediaDownloader: Send + Sync {
    async fn download(
        &self,
        provider: &Provider,
        attachment: &MessageAttachment,
    ) -> Result<DownloadedMedia>;
}

pub type DynMediaDownloader = Arc<dyn MediaDownloader>;

pub struct RealMediaDownloader {
    config: Config,
    client: reqwest::Client,
}

impl RealMediaDownloader {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl MediaDownloader for RealMediaDownloader {
    async fn download(
        &self,
        provider: &Provider,
        attachment: &MessageAttachment,
    ) -> Result<DownloadedMedia> {
        match provider {
            Provider::Telegram => self.download_telegram(attachment).await,
            Provider::Max => self.download_max(attachment).await,
        }
    }
}

impl RealMediaDownloader {
    async fn download_telegram(&self, attachment: &MessageAttachment) -> Result<DownloadedMedia> {
        let token =
            self.config.telegram_bot_token.as_deref().ok_or_else(|| {
                anyhow!("TELEGRAM_BOT_TOKEN is required to download Telegram images")
            })?;
        let file_response: TelegramFileResponse = self
            .client
            .get(format!("https://api.telegram.org/bot{token}/getFile"))
            .query(&[("file_id", attachment.provider_file_id.as_str())])
            .send()
            .await
            .context("failed to call Telegram getFile")?
            .error_for_status()
            .context("Telegram getFile returned an error status")?
            .json()
            .await
            .context("failed to decode Telegram getFile response")?;
        if !file_response.ok {
            return Err(anyhow!("Telegram getFile returned ok=false"));
        }
        let file_path = file_response
            .result
            .file_path
            .ok_or_else(|| anyhow!("Telegram getFile response did not include file_path"))?;
        let bytes = self
            .client
            .get(format!(
                "https://api.telegram.org/file/bot{token}/{file_path}"
            ))
            .send()
            .await
            .context("failed to download Telegram file")?
            .error_for_status()
            .context("Telegram file download returned an error status")?
            .bytes()
            .await
            .context("failed to read Telegram file bytes")?
            .to_vec();
        Ok(DownloadedMedia {
            bytes,
            mime_type: attachment.mime_type.clone(),
            provider_file_path: Some(file_path),
        })
    }

    async fn download_max(&self, attachment: &MessageAttachment) -> Result<DownloadedMedia> {
        let url = Url::parse(&attachment.provider_file_id)
            .context("MAX image attachment did not include a valid download URL")?;
        match url.scheme() {
            "http" | "https" => {}
            _ => return Err(anyhow!("MAX image attachment URL must use HTTP or HTTPS")),
        }
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .context("failed to download MAX image attachment")?
            .error_for_status()
            .context("MAX image download returned an error status")?;
        let response_mime = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or(value).trim().to_string())
            .filter(|value| !value.is_empty());
        let bytes = response
            .bytes()
            .await
            .context("failed to read MAX image bytes")?
            .to_vec();
        Ok(DownloadedMedia {
            bytes,
            mime_type: attachment.mime_type.clone().or(response_mime),
            provider_file_path: Some(url.path().to_string()),
        })
    }
}

#[derive(Debug, Deserialize)]
struct TelegramFileResponse {
    ok: bool,
    result: TelegramFile,
}

#[derive(Debug, Deserialize)]
struct TelegramFile {
    file_path: Option<String>,
}

#[cfg(test)]
pub struct FakeMediaDownloader {
    media: DownloadedMedia,
}

#[cfg(test)]
impl FakeMediaDownloader {
    pub fn new(media: DownloadedMedia) -> Self {
        Self { media }
    }
}

#[cfg(test)]
#[async_trait]
impl MediaDownloader for FakeMediaDownloader {
    async fn download(
        &self,
        _provider: &Provider,
        _attachment: &MessageAttachment,
    ) -> Result<DownloadedMedia> {
        Ok(self.media.clone())
    }
}
