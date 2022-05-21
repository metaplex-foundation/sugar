use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::task::JoinHandle;

use crate::config::{ConfigData, SugarConfig, UploadMethod};
use crate::upload::{
    assets::{AssetInfo, AssetPair, DataType},
    methods::*,
};

/// A trait for storage upload handlers.
#[async_trait]
pub trait StorageMethod {
    /// Prepare the upload of the specified media/metadata files. This generally
    /// involve checking if there is space/funds for the upload.
    async fn prepare(
        &self,
        sugar_config: &SugarConfig,
        assets: &HashMap<usize, AssetPair>,
        asset_indices: Vec<(DataType, &[usize])>,
    ) -> Result<()>;

    /// Upload the asset to the storage and return a tuple (`asset id`, `url`) if
    /// successful.
    fn upload_data(&self, asset_info: AssetInfo) -> JoinHandle<Result<(String, String)>>;
}

pub async fn initialize(
    sugar_config: &SugarConfig,
    config_data: &ConfigData,
) -> Result<Box<dyn StorageMethod>> {
    Ok(match config_data.upload_method {
        UploadMethod::AWS => {
            Box::new(AWSMethod::initialize(config_data).await?) as Box<dyn StorageMethod>
        }
        UploadMethod::Bundlr => {
            Box::new(BundlrMethod::initialize(sugar_config, config_data).await?)
                as Box<dyn StorageMethod>
        }
    })
}
