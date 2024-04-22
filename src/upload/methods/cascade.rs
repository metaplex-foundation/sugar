use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use reqwest::{
    header,
    multipart::{Form, Part},
    Client, StatusCode,
};
use tokio::time::{sleep, Duration};

use crate::{common::*, config::*, upload::*};

// API end point.
const CASCADE_API_URL: &str = "https://gateway-api.pastel.network/";
// Request time window (ms) to avoid the rate limit.
const REQUEST_WAIT: u64 = 10000;
// File size limit (100mb).
const FILE_SIZE_LIMIT: u64 = 100 * 1024 * 1024;
// Number of files per request limit.
const FILE_COUNT_LIMIT: u64 = 100;

pub enum CascadeStorageError {
    ApiError(Value),
}

/// response after an nft was stored
#[derive(Debug, Deserialize, Default)]
pub struct UploadResponse {
    /// id of the request
    pub request_id: String,
    /// status of the request
    pub request_status: String,
    /// stored nft data
    pub results: Vec<UploadResult>,
}

/// main obj that hold all the response data
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct UploadResult {
    pub result_id: String,
    pub result_status: String,
    pub original_file_ipfs_link: Option<String>,
    pub error: Option<String>,
}

pub struct CascadeStorageMethod {
    client: Arc<Client>,
}

impl CascadeStorageMethod {
    /// Initialize a new CascadeStorageHandler.
    pub async fn new(config_data: &ConfigData) -> Result<Self> {
        if let Some(api_key) = &config_data.cascade_api_key {
            let client_builder = Client::builder();

            let mut headers = header::HeaderMap::new();
            let mut api_key_mut = header::HeaderValue::from_str(api_key)?;
            api_key_mut.set_sensitive(true);
            headers.insert("Api_key", api_key_mut);

            let client = client_builder.default_headers(headers).build()?;

            let url = format!("{}/api/v1/cascade/gateway_requests", CASCADE_API_URL);
            let response = client.get(url).send().await?;

            match response.status() {
                StatusCode::OK => Ok(Self {
                    client: Arc::new(client),
                }),
                StatusCode::UNAUTHORIZED => Err(anyhow!("Invalid cascade api key.")),
                code => Err(anyhow!("Could not initialize cascade client: {code}")),
            }
        } else {
            Err(anyhow!("Missing 'CascadeApiKey' value in config file."))
        }
    }
}

#[async_trait]
impl Prepare for CascadeStorageMethod {
    /// Verifies that no file is larger than 100MB (upload of files larger than 100MB are
    /// not currently supported).
    async fn prepare(
        &self,
        _sugar_config: &SugarConfig,
        asset_pairs: &HashMap<isize, AssetPair>,
        asset_indices: Vec<(DataType, &[isize])>,
    ) -> Result<()> {
        for (data_type, indices) in asset_indices {
            for index in indices {
                let item = asset_pairs.get(index).unwrap();
                let size = match data_type {
                    DataType::Image => {
                        let path = Path::new(&item.image);
                        fs::metadata(path)?.len()
                    }
                    DataType::Animation => {
                        if let Some(animation) = &item.animation {
                            let path = Path::new(animation);
                            fs::metadata(path)?.len()
                        } else {
                            0
                        }
                    }
                    DataType::Metadata => {
                        let mock_uri = "x".repeat(MOCK_URI_SIZE);
                        let animation = if item.animation.is_some() {
                            Some(mock_uri.clone())
                        } else {
                            None
                        };

                        get_updated_metadata(&item.metadata, &mock_uri.clone(), &animation)?
                            .into_bytes()
                            .len() as u64
                    }
                };

                if size > FILE_SIZE_LIMIT {
                    return Err(anyhow!(
                        "File '{}' exceeds the current 100MB file size limit",
                        item.name,
                    ));
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Uploader for CascadeStorageMethod {
    /// Upload the data to Nft Storage
    async fn upload(
        &self,
        _sugar_config: &SugarConfig,
        cache: &mut Cache,
        data_type: DataType,
        assets: &mut Vec<AssetInfo>,
        progress: &ProgressBar,
        interrupted: Arc<AtomicBool>,
    ) -> Result<Vec<UploadError>> {
        let mut batches: Vec<Vec<&AssetInfo>> = Vec::new();
        let mut current: Vec<&AssetInfo> = Vec::new();
        let mut upload_size = 0;
        let mut upload_count = 0;

        for asset_info in assets {
            let size = match data_type {
                DataType::Image | DataType::Animation => {
                    let path = Path::new(&asset_info.content);
                    fs::metadata(path)?.len()
                }
                DataType::Metadata => {
                    let content = String::from(&asset_info.content);
                    content.into_bytes().len() as u64
                }
            };

            if (upload_size + size) > FILE_SIZE_LIMIT || (upload_count + 1) > FILE_COUNT_LIMIT {
                batches.push(current);
                current = Vec::new();
                upload_size = 0;
                upload_count = 0;
            }

            upload_size += size;
            upload_count += 1;
            current.push(asset_info);
        }
        // adds the last chunk (if there is one)
        if !current.is_empty() {
            batches.push(current);
        }

        let mut errors = Vec::new();
        // sets the length of the progress bar as the number of batches
        progress.set_length(batches.len() as u64);

        while !interrupted.load(Ordering::SeqCst) && !batches.is_empty() {
            let batch = batches.remove(0);
            let mut form = Form::new();

            for asset_info in &batch {
                let data = match asset_info.data_type {
                    DataType::Image | DataType::Animation => fs::read(&asset_info.content)?,
                    DataType::Metadata => {
                        let content = String::from(&asset_info.content);
                        content.into_bytes()
                    }
                };

                let file = Part::bytes(data)
                    .file_name(asset_info.name.clone())
                    .mime_str(asset_info.content_type.as_str())?;
                form = form.part("files", file);
            }

            let response = self
                .client
                .post(format!(
                    "{CASCADE_API_URL}/api/v1/cascade?make_publicly_accessible=true"
                ))
                .multipart(form)
                .send()
                .await?;
            let status = response.status();

            if status.is_success() {
                let body = response.json::<Value>().await?;
                let response: UploadResponse = serde_json::from_value(body)?;

                // updates the cache content

                for asset_info in batch {
                    let id = asset_info.asset_id.clone();
                    if response.results[0].original_file_ipfs_link.is_some() {
                        let uri = response.results[0].original_file_ipfs_link.clone().unwrap();
                        // cache item to update
                        let item = cache.items.get_mut(&id).unwrap();

                        match data_type {
                            DataType::Image => item.image_link = uri,
                            DataType::Metadata => item.metadata_link = uri,
                            DataType::Animation => item.animation_link = Some(uri),
                        }
                    } else {
                        errors.push(UploadError::SendDataFailed(format!(
                            "Error uploading batch ({})",
                            response.results[0].result_status
                        )));
                    }
                }
                // syncs cache (checkpoint)
                cache.sync_file()?;
                // updates the progress bar
                progress.inc(1);
            } else {
                let body = response.json::<Value>().await?;
                let response: UploadResponse = serde_json::from_value(body)?;
                if !response.results.is_empty() {
                    if response.results[0].error.is_some() {
                        errors.push(UploadError::SendDataFailed(format!(
                            "Error uploading batch ({}): {}",
                            status,
                            response.results[0].error.clone().unwrap()
                        )));
                    } else {
                        errors.push(UploadError::SendDataFailed(format!(
                            "Error uploading batch ({}): {}",
                            status, response.results[0].result_status
                        )));
                    }
                } else {
                    errors.push(UploadError::SendDataFailed(format!(
                        "Error uploading batch ({}): {}",
                        status, response.request_status
                    )));
                }
            }
            if !batches.is_empty() {
                // wait to minimize the chance of getting caught by the rate limit
                sleep(Duration::from_millis(REQUEST_WAIT)).await;
            }
        }

        Ok(errors)
    }
}
