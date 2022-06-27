use anyhow::Result;
use async_trait::async_trait;
use console::style;
use futures::future::select_all;
pub use indicatif::ProgressBar;
use std::collections::HashMap;
use std::{
    cmp,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::task::JoinHandle;

use crate::cache::Cache;
use crate::config::{ConfigData, SugarConfig, UploadMethod};
use crate::constants::PARALLEL_LIMIT;
use crate::upload::{
    assets::{AssetPair, DataType},
    methods::*,
    UploadError,
};

/// Size of the mock media URI for cost calculation
pub const MOCK_URI_SIZE: usize = 100;

/// Struct representing an asset ready for upload. An `AssetInfo` can represent
/// a physical file, in which case the `content` will correspond to the name
/// of the file, or an in-memory asset, in which case the `content` will correspond
/// to the content of the asset.
///
/// For example, for image files, the `content` contains the name of the file on the
/// file system. In the case of the json metadata file, the `content` contains the json
/// string of the metadata.
pub struct AssetInfo {
    pub asset_id: String,
    pub name: String,
    pub content: String,
    pub data_type: DataType,
    pub content_type: String,
}

#[async_trait]
pub trait Prepare {
    /// Prepare the upload of the specified media/metadata files. This generally
    /// involve checking if there is space/funds for the upload.
    async fn prepare(
        &self,
        sugar_config: &SugarConfig,
        asset_pairs: &HashMap<isize, AssetPair>,
        asset_indices: Vec<(DataType, &[isize])>,
    ) -> Result<()>;
}

/// A trait for storage upload handlers. This trait should be implemented directly by
/// upload methods that do not support parallel uploads (threading).
#[async_trait]
pub trait Uploader: Prepare {
    /// Upload all assets to the storage. This function will be called to upload
    /// each type of asset separately.
    async fn upload(
        &self,
        sugar_config: &SugarConfig,
        cache: &mut Cache,
        data_type: DataType,
        assets: &mut Vec<AssetInfo>,
        progress: &ProgressBar,
        interrupted: Arc<AtomicBool>,
    ) -> Result<Vec<UploadError>>;
}

/// A trait for parallel storage upload handlers. THis trait should be implemented by
/// methods that support parallel uploads (threading). In this case, the return of
/// the `upload` method should be a `JoinHandle` to the thread that is responsible to upload
/// the asset.
#[async_trait]
pub trait ParallelUploader: Uploader + Send + Sync {
    fn upload_asset(&self, assets: AssetInfo) -> JoinHandle<Result<(String, String)>>;
}

#[async_trait]
impl<T: ParallelUploader> Uploader for T {
    async fn upload(
        &self,
        _sugar_config: &SugarConfig,
        cache: &mut Cache,
        data_type: DataType,
        assets: &mut Vec<AssetInfo>,
        progress: &ProgressBar,
        interrupted: Arc<AtomicBool>,
    ) -> Result<Vec<UploadError>> {
        let mut handles = Vec::new();

        for task in assets.drain(0..cmp::min(assets.len(), PARALLEL_LIMIT)) {
            handles.push(self.upload_asset(task));
        }

        let mut errors = Vec::new();

        while !interrupted.load(Ordering::SeqCst) && !handles.is_empty() {
            match select_all(handles).await {
                (Ok(res), _index, remaining) => {
                    // independently if the upload was successful or not
                    // we continue to try the remaining ones
                    handles = remaining;
                    if res.is_ok() {
                        let val = res?;
                        let link = val.clone().1;
                        // cache item to update
                        let item = cache.items.0.get_mut(&val.0).unwrap();
                        match data_type {
                            DataType::Image => item.image_link = link,
                            DataType::Metadata => item.metadata_link = link,
                            DataType::Animation => item.animation_link = Some(link),
                        }
                        // updates the progress bar
                        progress.inc(1);
                    } else {
                        // user will need to retry the upload
                        errors.push(UploadError::SendDataFailed(format!(
                            "Upload error: {:?}",
                            res.err().unwrap()
                        )));
                    }
                }
                (Err(err), _index, remaining) => {
                    errors.push(UploadError::SendDataFailed(format!(
                        "Upload error: {:?}",
                        err
                    )));
                    // ignoring all errors
                    handles = remaining;
                }
            }
            if !assets.is_empty() {
                // if we are half way through, let spawn more transactions
                if (PARALLEL_LIMIT - handles.len()) > (PARALLEL_LIMIT / 2) {
                    // syncs cache (checkpoint)
                    cache.sync_file()?;
                    for task in assets.drain(0..cmp::min(assets.len(), PARALLEL_LIMIT / 2)) {
                        handles.push(self.upload_asset(task));
                    }
                }
            }
        }

        if errors.is_empty() && !assets.is_empty() {
            progress.abandon_with_message(format!("{}", style("Upload aborted ").red().bold()));
            return Err(
                UploadError::SendDataFailed("Not all files were uploaded.".to_string()).into(),
            );
        }

        Ok(errors)
    }
}

/// Factory function for uploader objects.
///
/// Returns a new uploader trait object based on the configuration `uploadMethod`.
pub async fn initialize(
    sugar_config: &SugarConfig,
    config_data: &ConfigData,
) -> Result<Box<dyn Uploader>> {
    Ok(match config_data.upload_method {
        UploadMethod::AWS => Box::new(AWSMethod::new(config_data).await?) as Box<dyn Uploader>,
        UploadMethod::Bundlr => {
            Box::new(BundlrMethod::new(sugar_config, config_data).await?) as Box<dyn Uploader>
        }
        UploadMethod::ShadowDrive => {
            Box::new(SHDWMethod::new(sugar_config, config_data)?) as Box<dyn Uploader>
        }
        UploadMethod::NftStorage => {
            Box::new(NftStorageMethod::initialize(config_data).await?) as Box<dyn Uploader>
        }
    })
}
