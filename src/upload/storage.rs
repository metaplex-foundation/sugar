use anyhow::Result;
use std::collections::HashMap;
use tokio::task::JoinHandle;

use crate::config::{ConfigData, SugarConfig, UploadMethod};
use crate::upload::{
    assets::{AssetInfo, AssetPair, DataType},
    methods::*,
};

pub enum Storage {
    AWS(AWSMethod),
    Bundlr(BundlrMethod),
}

pub async fn initialize(sugar_config: &SugarConfig, config_data: &ConfigData) -> Result<Storage> {
    Ok(match config_data.upload_method {
        UploadMethod::AWS => Storage::AWS(AWSMethod::initialize(config_data).await?),
        UploadMethod::Bundlr => {
            Storage::Bundlr(BundlrMethod::initialize(sugar_config, config_data).await?)
        }
    })
}

pub async fn prepare_upload(
    dispatcher: &Storage,
    sugar_config: &SugarConfig,
    assets: &HashMap<usize, AssetPair>,
    asset_indices: Vec<(DataType, &[usize])>,
) -> Result<()> {
    match dispatcher {
        Storage::Bundlr(bundlr) => bundlr.prepare(sugar_config, assets, asset_indices).await,
        _ => Ok(()),
    }
}

pub fn upload_data(
    dispatcher: &Storage,
    asset_info: AssetInfo,
) -> JoinHandle<Result<(String, String)>> {
    match dispatcher {
        Storage::AWS(aws) => {
            let client = aws.aws_client.clone();
            let bucket = aws.bucket.clone();
            tokio::spawn(async move { AWSMethod::send(client, bucket, asset_info).await })
        }
        Storage::Bundlr(bundlr) => {
            let client = bundlr.client.clone();
            let tag = bundlr.sugar_tag.clone();

            tokio::spawn(async move { BundlrMethod::send(client, tag, asset_info).await })
        }
    }
}
