use std::{fs, sync::Arc};

use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use tokio::task::JoinHandle;

use crate::{
    common::*,
    config::*,
    upload::{
        assets::{AssetPair, DataType},
        uploader::{AssetInfo, ParallelUploader, Prepare},
        UploadError,
    },
};

// Maximum number of times to retry each individual upload.
const MAX_RETRY: usize = 3;
// Base URL for the Sdrive API.
const BASE_URL: &str = "https://sdrive.app/api/v3";

#[derive(Deserialize)]
struct UploadResponse {
    status: String,
    permalink: String,
}

pub struct SdriveMethod {
    pub config: Arc<String>,
}

impl SdriveMethod {
    pub async fn new(config_data: &ConfigData) -> Result<Self> {
        if let Some(config) = &config_data.sdrive_api_key {
            Ok(Self {
                config: Arc::new(config.clone()),
            })
        } else {
            Err(anyhow!("Missing Sdrive values in config file."))
        }
    }

    async fn send(apikey: String, asset_info: AssetInfo) -> Result<(String, String)> {
        let data = match asset_info.data_type {
            DataType::Image => fs::read(&asset_info.content)?,
            DataType::Metadata => asset_info.content.into_bytes(),
            DataType::Animation => fs::read(&asset_info.content)?,
        };
        let data_clone = data.clone(); // Clone the data outside the loop
        let apikey_clone = apikey.clone(); // Clone the apikey outside the loop

        let http_client = reqwest::Client::new();
        let mut retries = 0;

        loop {
            let file = Part::bytes(data_clone.clone()) // Use the cloned data
                .file_name(asset_info.name.clone())
                .mime_str(asset_info.content_type.as_str())?;

            let form = Form::new()
                .part("fileupload", file)
                .text("apikey", apikey_clone.clone()); // Use the cloned apikey

            let response = http_client
                .post(format!("{}/upload", &BASE_URL))
                .multipart(form)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let upload_response: UploadResponse = resp.json().await?;
                        if upload_response.status == "success" {
                            return Ok((asset_info.asset_id.clone(), upload_response.permalink));
                        }
                    } else {
                        return Err(anyhow!(UploadError::SendDataFailed(format!(
                            "Error uploading file ({}): {}",
                            resp.status(),
                            resp.text().await?,
                        ))));
                    }
                }
                Err(_) => {
                    retries += 1;
                    if retries >= MAX_RETRY {
                        return Err(anyhow::anyhow!("Failed to upload to Sdrive."));
                    }
                }
            }
        }
    }
}

#[async_trait]
impl Prepare for SdriveMethod {
    async fn prepare(
        &self,
        _sugar_config: &SugarConfig,
        _asset_pairs: &HashMap<isize, AssetPair>,
        _asset_indices: Vec<(DataType, &[isize])>,
    ) -> Result<()> {
        // nothing to do here
        Ok(())
    }
}

#[async_trait]
impl ParallelUploader for SdriveMethod {
    fn upload_asset(&self, asset_info: AssetInfo) -> JoinHandle<Result<(String, String)>> {
        let apikey = self.config.clone();

        tokio::spawn(async move { SdriveMethod::send(apikey.to_string(), asset_info).await })
    }
}
