use anchor_client::solana_sdk::{
    pubkey::Pubkey, signature::keypair::Keypair, signer::Signer, system_program,
};
use anchor_lang::{prelude::*, InstructionData};
use anyhow::Result;
use async_trait::async_trait;
use base64::encode;
use data_encoding::HEXLOWER;
use reqwest::multipart::{Form, Part};
use ring::digest::{Context, SHA256};
use serde::{Deserialize, Serialize};
use shadow_drive_user_staking::accounts::StoreFile as StoreFileAccount;
use shadow_drive_user_staking::instruction::StoreFile as StoreFileInstruction;
use shadow_drive_user_staking::instructions::initialize_account::StorageAccount;
use solana_program::instruction::Instruction;
use std::{fs, sync::Arc};
use tokio::task::JoinHandle;

use crate::upload::{
    assets::{get_updated_metadata, AssetPair, DataType},
    storage::{AssetInfo, StorageMethod, MOCK_URI_SIZE},
};
use crate::{common::*, config::*};

// Constant use to retrieve the "user-info" account.
const USER_INFO: &str = "user-info";

// Constant use to retrieve the "storage-config" account.
const STORAGE_CONFIG: &str = "storage-config";

// Token mint pubkey.
const TOKEN_MINT: Pubkey = solana_program::pubkey!("SHDWyBxihqiCj6YekG2GUr7wqKLeLAMK1gHZck9pL6y");

// Uploader pubkey.
const UPLOADER: Pubkey = solana_program::pubkey!("972oJTFyjmVNsWM4GHEGPWUomAiJf2qrVotLtwnKmWem");

// Shadow Drive endpoin.
const SHDW_DRIVE_ENDPOINT: &str = "https://shadow-storage.genesysgo.net";

pub struct SHDWMethod {
    sugar_config: Arc<SugarConfig>,
    user_info: Pubkey,
    storage_pubkey: Pubkey,
    storage_account: Arc<StorageAccount>,
}

#[derive(Serialize, Deserialize)]
struct UploadResponse {
    finalized_location: String,
    transaction_signature: String,
}

impl SHDWMethod {
    pub async fn initialize(
        sugar_config: &SugarConfig,
        _config_data: &ConfigData,
    ) -> Result<SHDWMethod> {
        let seed = &[
            USER_INFO.as_bytes(),
            &sugar_config.keypair.pubkey().to_bytes(),
        ];
        let (user_info, _bump) = Pubkey::find_program_address(seed, &shadow_drive_user_staking::ID);

        // load the storage account
        let client = setup_client(sugar_config)?;
        let program = client.program(shadow_drive_user_staking::ID);

        // TODO: move this to the configuration file or list all available for
        // the user to choose one
        let storage_pubkey = Pubkey::from_str("DCG6qThfZE8xbM72RoFRLwRSrhNVjeWE1gVPPCGvLYSS")?;

        let storage = program.rpc().get_account_data(&storage_pubkey)?;
        let storage_account = StorageAccount::try_deserialize(&mut storage.as_slice())?;

        let keypair = Keypair::from_bytes(&sugar_config.keypair.to_bytes())?;
        let config = Arc::new(SugarConfig {
            keypair,
            rpc_url: sugar_config.rpc_url.clone(),
        });

        Ok(SHDWMethod {
            sugar_config: config,
            user_info,
            storage_pubkey,
            storage_account: Arc::new(storage_account),
        })
    }

    async fn send(
        sugar_config: Arc<SugarConfig>,
        user_info: Pubkey,
        storage_pubkey: Pubkey,
        storage_account: Arc<StorageAccount>,
        asset_info: AssetInfo,
    ) -> Result<(String, String)> {
        let data = match asset_info.data_type {
            DataType::Image => fs::read(&asset_info.content)?,
            DataType::Metadata => asset_info.content.into_bytes(),
            DataType::Animation => fs::read(&asset_info.content)?,
        };

        let mut context = Context::new(&SHA256);
        context.update(&data);
        let hash = HEXLOWER.encode(context.finish().as_ref());
        
        // TODO: the use of the counter here does not allow parallel upload, since
        // it will generated the same PDA address for different files - we could use
        // the id of the asset as the 'counter'
        let counter = storage_account.init_counter.to_le_bytes();
        let seeds = &[storage_pubkey.as_ref(), counter.as_ref()];

        let (file_account, _bump) =
            Pubkey::find_program_address(seeds, &shadow_drive_user_staking::ID);

        let (storage_config, _bump) = Pubkey::find_program_address(
            &[STORAGE_CONFIG.as_bytes()],
            &shadow_drive_user_staking::ID,
        );

        let encoded = {
            let keypair = bs58::encode(sugar_config.keypair.to_bytes()).into_string();
            let payer = Keypair::from_base58_string(&keypair);

            let client = setup_client(&sugar_config)?;
            let program = client.program(shadow_drive_user_staking::ID);

            let storage_instruction = StoreFileInstruction {
                filename: asset_info.name.clone(),
                sha256_hash: hash,
                size: data.len() as u64,
            };

            let accounts = StoreFileAccount {
                storage_config,
                storage_account: storage_pubkey,
                file: file_account,
                user_info,
                owner: storage_account.owner_1,
                uploader: UPLOADER,
                token_mint: TOKEN_MINT,
                system_program: system_program::id(),
            };

            let instruction = Instruction {
                program_id: shadow_drive_user_staking::ID,
                data: storage_instruction.data(),
                accounts: accounts.to_account_metas(None),
            };

            let mut tx = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));

            let blockhash = program.rpc().get_latest_blockhash()?;
            tx.partial_sign(&[&payer], blockhash);

            bincode::serialize(&tx)?
        };

        let http_client = reqwest::Client::new();
        let file = Part::bytes(data)
            .file_name(asset_info.name)
            .mime_str(asset_info.content_type.as_str())?;
        let form = Form::new().part("file", file).text(
            "transaction",
            encode(serde_json::to_string(&encoded)?.as_bytes()),
        );

        let response = http_client
            .post(format!("{SHDW_DRIVE_ENDPOINT}/upload"))
            .multipart(form)
            .send()
            .await?;

        if response.status().is_success() {
            let link = response.json::<UploadResponse>().await?.finalized_location;
            Ok((asset_info.asset_id, link))
        } else {
            let error = response.text().await?;
            Err(anyhow!(format!("Error uploading file: {error}")))
        }
    }
}

#[async_trait]
impl StorageMethod for SHDWMethod {
    async fn prepare(
        &self,
        _sugar_config: &SugarConfig,
        assets: &HashMap<usize, AssetPair>,
        asset_indices: Vec<(DataType, &[usize])>,
    ) -> Result<()> {
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

        if self.storage_account.storage_available < total_size {
            let required = total_size - self.storage_account.storage_available;
            return Err(anyhow!(
                "Insufficient storage space (additional {required} bytes required)"
            ));
        }

        // we are good to go
        Ok(())
    }

    fn upload_data(&self, asset_info: AssetInfo) -> JoinHandle<Result<(String, String)>> {
        // TODO: data has a 1GB limit, file name 32 bytes long limit
        // (need to add validation)
        let sugar_config = self.sugar_config.clone();
        let user_info = self.user_info;
        let storage_pubkey = self.storage_pubkey;
        let storage_account = self.storage_account.clone();

        tokio::spawn(async move {
            SHDWMethod::send(
                sugar_config,
                user_info,
                storage_pubkey,
                storage_account,
                asset_info,
            )
            .await
        })
    }
}
