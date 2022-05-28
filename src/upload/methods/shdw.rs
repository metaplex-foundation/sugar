use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_lang::AnchorDeserialize;
use anyhow::Result;
use async_trait::async_trait;
use shadow_drive_user_staking::instructions::initialize_account::{StorageAccount, UserInfo};
use tokio::task::JoinHandle;

use crate::upload::{
    assets::{AssetPair, DataType, get_updated_metadata},
    storage::{AssetInfo, MOCK_URI_SIZE, StorageMethod},
};
use crate::{common::*, config::*};

const USER_INFO: &str = "user-info";

pub struct SHDWMethod {
    pub user_info: Pubkey,
}

impl SHDWMethod {
    pub async fn initialize(
        sugar_config: &SugarConfig,
        _config_data: &ConfigData,
    ) -> Result<SHDWMethod> {
        let seed = &[USER_INFO.as_bytes(), &sugar_config.keypair.to_bytes()];
        let (user_info, bump) = Pubkey::find_program_address(seed, &shadow_drive_user_staking::ID);

        Ok(SHDWMethod { user_info })
    }
}

#[async_trait]
impl StorageMethod for SHDWMethod {
    async fn prepare(
        &self,
        sugar_config: &SugarConfig,
        assets: &HashMap<usize, AssetPair>,
        asset_indices: Vec<(DataType, &[usize])>,
    ) -> Result<()> {
        let client = setup_client(sugar_config)?;
        let program = client.program(shadow_drive_user_staking::ID);
        let account = program
            .rpc()
            .get_account_with_commitment(&self.user_info, CommitmentConfig::confirmed())?;

        let _account_seed = if let Some(_account) = account.value {
            let data = program.rpc().get_account_data(&self.user_info)?;
            let user_info_data: UserInfo = UserInfo::deserialize(&mut data.as_slice())?;
            user_info_data.account_counter
        } else {
            // user_info not initialized, default to 0
            // for account seed
            0
        };

        // load the storage account

        let storage_pubkey = Pubkey::from_str("N4f6zftYsuu4yT7icsjLwh4i6pB1zvvKbseHj2NmSQw")?;

        let storage = program.rpc().get_account_data(&storage_pubkey)?;
        let storage_account = StorageAccount::deserialize(&mut storage.as_slice())?;

        // calculates the size of the files to upload

        let mut total_size = 0;

        for (data_type, indices) in asset_indices {
            match data_type {
                DataType::Image => {
                    for index in indices {
                        let item = assets.get(index).unwrap();
                        let path = Path::new(&item.image);
                        total_size += std::fs::metadata(path)?.len();
                    }
                }
                DataType::Animation => {
                    for index in indices {
                        let item = assets.get(index).unwrap();

                        if let Some(animation) = &item.animation {
                            let path = Path::new(animation);
                            total_size += std::fs::metadata(path)?.len();
                        }
                    }
                }
                DataType::Metadata => {
                    let mock_uri = "x".repeat(MOCK_URI_SIZE);

                    for index in indices {
                        let item = assets.get(index).unwrap();
                        let animation = if item.animation.is_some() {
                            Some(mock_uri.clone())
                        } else {
                            None
                        };

                        total_size +=
                            get_updated_metadata(&item.metadata, &mock_uri.clone(), &animation)?
                                .into_bytes()
                                .len() as u64;
                    }
                }
            }
        }

        info!("Total upload size: {}", total_size);

        if storage_account.storage_available < total_size {
            // needs to increase storage
        }

        // we are good to go
        Ok(())
    }

    fn upload_data(&self, _asset_info: AssetInfo) -> JoinHandle<Result<(String, String)>> {
        todo!();
    }
}
