use aws_sdk_s3::{types::ByteStream, Client};
use bs58;
use std::{fs, sync::Arc};

use crate::upload::assets::{get_updated_metadata, AssetInfo, DataType};
use crate::{common::*, config::*};

pub struct AWSMethod {
    pub aws_client: Arc<Client>,
    pub bucket: String,
}

impl AWSMethod {
    pub async fn initialize(config_data: &ConfigData) -> Result<AWSMethod> {
        let shared_config = aws_config::load_from_env().await;
        let client = Client::new(&shared_config);

        if let Some(aws_s3_bucket) = &config_data.aws_s3_bucket {
            Ok(AWSMethod {
                aws_client: Arc::new(client),
                bucket: aws_s3_bucket.to_string(),
            })
        } else {
            Err(anyhow!("Missing 'awsS3Bucket' value in config file."))
        }
    }

    pub async fn send(
        client: Arc<Client>,
        bucket: String,
        asset_info: AssetInfo,
    ) -> Result<(String, String)> {
        let data = match asset_info.data_type {
            DataType::Media => fs::read(&asset_info.file_path)?,
            DataType::Metadata => {
                // replaces the media link without modifying the original file to avoid
                // changing the hash of the metadata file
                get_updated_metadata(&asset_info.file_path, &asset_info.media_link)?.into_bytes()
            }
        };

        let key = bs58::encode(&asset_info.file_path).into_string();

        client
            .put_object()
            .bucket(&bucket)
            .key(&key)
            .body(ByteStream::from(data))
            .content_type(asset_info.content_type)
            .send()
            .await?;

        let link = format!("https://{}.s3.amazonaws.com/{}", bucket, key);

        Ok((asset_info.asset_id, link))
    }
}
