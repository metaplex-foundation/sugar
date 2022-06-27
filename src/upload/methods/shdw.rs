use anchor_client::solana_client::rpc_client::serialize_and_encode;
use anchor_client::solana_sdk::{pubkey::Pubkey, signer::Signer, system_program};
use anchor_lang::{prelude::*, InstructionData};
use anyhow::Result;
use async_trait::async_trait;
use console::style;
use data_encoding::HEXLOWER;
pub use indicatif::ProgressBar;
use reqwest::{
    multipart::{Form, Part},
    StatusCode,
};
use ring::digest::{Context, SHA256};
use serde::{Deserialize, Serialize};
use shadow_drive_user_staking::accounts::EditFile as EditFileAccount;
use shadow_drive_user_staking::accounts::StoreFile as StoreFileAccount;
use shadow_drive_user_staking::instruction::EditFile as EditFileInstruction;
use shadow_drive_user_staking::instruction::StoreFile as StoreFileInstruction;
use shadow_drive_user_staking::instructions::initialize_account::StorageAccount;
use solana_program::instruction::Instruction;
use solana_transaction_status::UiTransactionEncoding;
use std::{
    collections::hash_map::HashMap,
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::time::{sleep, Duration};

use crate::upload::{
    assets::{get_updated_metadata, AssetPair, DataType},
    uploader::{AssetInfo, Prepare, Uploader, MOCK_URI_SIZE},
    UploadError,
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

// Shadow Drive endpoint.
const SHDW_DRIVE_ENDPOINT: &str = "https://shadow-storage.genesysgo.net";

// Shadow Drive files location.
const SHDW_DRIVE_LOCATION: &str = "https://shdw-drive.genesysgo.net";

// Number of instructions limit.
const INSTRUCTION_COUNT_LIMIT: usize = 5;

// Number of bytes limit on file names per transaction.
const FILE_NAME_LIMIT: usize = 154;

/// The number os retries to fetch the storage account data (MAX_RETRY * DELAY_UNTIL_RETRY ms limit)
const MAX_RETRY: u64 = 120;

/// Time (ms) to wait until next try
const DELAY_UNTIL_RETRY: u64 = 1000;

pub struct SHDWMethod {
    user_info: Pubkey,
    storage_pubkey: Pubkey,
    storage_account: StorageAccount,
}

#[derive(Serialize, Deserialize)]
struct UploadResponse {
    finalized_locations: Vec<String>,
    transaction_signature: String,
}

impl SHDWMethod {
    pub fn new(sugar_config: &SugarConfig, config_data: &ConfigData) -> Result<Self> {
        let seed = &[
            USER_INFO.as_bytes(),
            &sugar_config.keypair.pubkey().to_bytes(),
        ];
        let (user_info, _bump) = Pubkey::find_program_address(seed, &shadow_drive_user_staking::ID);

        // load the storage account
        let client = setup_client(sugar_config)?;
        let program = client.program(shadow_drive_user_staking::ID);

        let storage_pubkey = if let Some(pubkey) = &config_data.shdw_storage {
            *pubkey
        } else {
            return Err(anyhow!("Missing 'shdwStorage' value in config file."));
        };

        let storage = program.rpc().get_account_data(&storage_pubkey)?;
        let storage_account = StorageAccount::try_deserialize(&mut storage.as_slice())?;

        Ok(Self {
            user_info,
            storage_pubkey,
            storage_account,
        })
    }

    async fn process_upload(
        &self,
        sugar_config: &SugarConfig,
        cache: &mut Cache,
        data_type: &DataType,
        assets: &mut Vec<Vec<&AssetInfo>>,
        progress: &ProgressBar,
        interrupted: Arc<AtomicBool>,
    ) -> Result<Vec<UploadError>> {
        let (storage_config, _bump) = Pubkey::find_program_address(
            &[STORAGE_CONFIG.as_bytes()],
            &shadow_drive_user_staking::ID,
        );

        let http_client = reqwest::Client::new();
        let mut errors = Vec::new();
        let mut init_counter = self.storage_account.init_counter;

        while !interrupted.load(Ordering::SeqCst) && !assets.is_empty() {
            // need to check that the value on chain is up to date
            if init_counter != self.storage_account.init_counter {
                let mut up_to_date = false;

                for _i in 0..MAX_RETRY {
                    let on_chain = {
                        let client = setup_client(sugar_config)?;
                        let program = client.program(shadow_drive_user_staking::ID);

                        let storage = program.rpc().get_account_data(&self.storage_pubkey)?;
                        let storage_account =
                            StorageAccount::try_deserialize(&mut storage.as_slice())?;

                        storage_account.init_counter
                    };

                    if init_counter == on_chain {
                        up_to_date = true;
                        break;
                    }

                    sleep(Duration::from_millis(DELAY_UNTIL_RETRY)).await;
                }

                if !up_to_date {
                    return Err(anyhow!(
                        "Could not confirm storage account information on-chain"
                    ));
                }
            }

            let batch = assets.remove(0);
            let mut instructions = vec![];
            let mut form = Form::new();

            let encoded = {
                let client = setup_client(sugar_config)?;
                let program = client.program(shadow_drive_user_staking::ID);

                for asset_info in &batch {
                    let data = match asset_info.data_type {
                        DataType::Image => fs::read(&asset_info.content)?,
                        DataType::Metadata => asset_info.content.clone().into_bytes(),
                        DataType::Animation => fs::read(&asset_info.content)?,
                    };
                    let mut context = Context::new(&SHA256);
                    context.update(&data);
                    let hash = HEXLOWER.encode(context.finish().as_ref());

                    let counter = init_counter.to_le_bytes();
                    let seeds = &[self.storage_pubkey.as_ref(), counter.as_ref()];
                    let (file_account, _bump) =
                        Pubkey::find_program_address(seeds, &shadow_drive_user_staking::ID);
                    let storage_instruction = StoreFileInstruction {
                        filename: asset_info.name.clone(),
                        sha256_hash: hash,
                        size: data.len() as u64,
                    };
                    let accounts = StoreFileAccount {
                        storage_config,
                        storage_account: self.storage_pubkey,
                        file: file_account,
                        user_info: self.user_info,
                        owner: self.storage_account.owner_1,
                        uploader: UPLOADER,
                        token_mint: TOKEN_MINT,
                        system_program: system_program::id(),
                    };
                    let instruction = Instruction {
                        program_id: shadow_drive_user_staking::ID,
                        data: storage_instruction.data(),
                        accounts: accounts.to_account_metas(None),
                    };

                    instructions.push(instruction);

                    let file = Part::bytes(data)
                        .file_name(asset_info.name.clone())
                        .mime_str(asset_info.content_type.as_str())?;
                    form = form.part("file", file);

                    init_counter += 1;
                }
                let mut tx = Transaction::new_with_payer(
                    &instructions,
                    Some(&sugar_config.keypair.pubkey()),
                );
                let blockhash = program.rpc().get_latest_blockhash()?;
                tx.partial_sign(&[&sugar_config.keypair], blockhash);
                // serializes the transaction
                serialize_and_encode(&tx, UiTransactionEncoding::Base64)?
            };

            form = form.part("transaction", Part::text(encoded));

            let response = http_client
                .post(format!("{SHDW_DRIVE_ENDPOINT}/upload-batch"))
                .multipart(form)
                .send()
                .await?;
            let status = response.status();

            if status.is_success() {
                let locations = response.json::<UploadResponse>().await?.finalized_locations;

                // updates the cache content

                for (index, asset_info) in batch.iter().enumerate() {
                    let id = asset_info.asset_id.clone();
                    let uri = locations[index].to_string();
                    // cache item to update
                    let item = cache.items.0.get_mut(&id).unwrap();

                    match data_type {
                        DataType::Image => item.image_link = uri,
                        DataType::Metadata => item.metadata_link = uri,
                        DataType::Animation => item.animation_link = Some(uri),
                    }
                    // updates the progress bar
                    progress.inc(1);
                }
                // syncs cache (checkpoint)
                cache.sync_file()?;
            } else {
                let error = response.json::<HashMap<String, String>>().await?;
                let message = if let Some(m) = error.get("error") {
                    m.to_string()
                } else {
                    format!("Error uploading batch (http status {})", status)
                };

                errors.push(UploadError::SendDataFailed(message));
            }
        }

        Ok(errors)
    }

    async fn process_edit(
        &self,
        sugar_config: &SugarConfig,
        cache: &mut Cache,
        data_type: &DataType,
        assets: &mut Vec<&AssetInfo>,
        progress: &ProgressBar,
        interrupted: Arc<AtomicBool>,
    ) -> Result<Vec<UploadError>> {
        let (storage_config, _bump) = Pubkey::find_program_address(
            &[STORAGE_CONFIG.as_bytes()],
            &shadow_drive_user_staking::ID,
        );

        let http_client = reqwest::Client::new();
        let mut errors = Vec::new();

        while !interrupted.load(Ordering::SeqCst) && !assets.is_empty() {
            let asset_info = assets.remove(0);

            let location = format!(
                "{}/{}/{}",
                SHDW_DRIVE_LOCATION, &self.storage_pubkey, &asset_info.name,
            );

            let mut map = HashMap::new();
            map.insert("location", location);

            let response = http_client
                .post(format!("{SHDW_DRIVE_ENDPOINT}/get-object-data"))
                .json(&map)
                .send()
                .await?;
            let status = response.status();

            if status.is_success() {
                let text = response.json::<HashMap<String, String>>().await?;
                if let Some(pubkey) = text.get("owner-account-pubkey") {
                    let owner = Pubkey::from_str(pubkey)?;

                    if sugar_config.keypair.pubkey() == owner {
                        let file_account =
                            Pubkey::from_str(text.get("file-account-pubkey").unwrap())?;

                        let data = match asset_info.data_type {
                            DataType::Image => fs::read(&asset_info.content)?,
                            DataType::Metadata => asset_info.content.clone().into_bytes(),
                            DataType::Animation => fs::read(&asset_info.content)?,
                        };
                        let mut context = Context::new(&SHA256);
                        context.update(&data);
                        let hash = HEXLOWER.encode(context.finish().as_ref());

                        let encoded = {
                            let client = setup_client(sugar_config)?;
                            let program = client.program(shadow_drive_user_staking::ID);
                            let edit_instruction = EditFileInstruction {
                                sha256_hash: hash,
                                size: data.len() as u64,
                            };
                            let accounts = EditFileAccount {
                                storage_config,
                                storage_account: self.storage_pubkey,
                                file: file_account,
                                owner: self.storage_account.owner_1,
                                uploader: UPLOADER,
                                token_mint: TOKEN_MINT,
                                system_program: system_program::id(),
                            };
                            let instruction = Instruction {
                                program_id: shadow_drive_user_staking::ID,
                                data: edit_instruction.data(),
                                accounts: accounts.to_account_metas(None),
                            };

                            let mut tx = Transaction::new_with_payer(
                                &[instruction],
                                Some(&sugar_config.keypair.pubkey()),
                            );
                            let blockhash = program.rpc().get_latest_blockhash()?;
                            tx.partial_sign(&[&sugar_config.keypair], blockhash);
                            // serializes the transaction
                            serialize_and_encode(&tx, UiTransactionEncoding::Base64)?
                        };

                        let file = Part::bytes(data)
                            .file_name(asset_info.name.clone())
                            .mime_str(asset_info.content_type.as_str())?;
                        let form = Form::new()
                            .part("file", file)
                            .part("transaction", Part::text(encoded));

                        let response = http_client
                            .post(format!("{SHDW_DRIVE_ENDPOINT}/edit"))
                            .multipart(form)
                            .send()
                            .await?;
                        let status = response.status();
                        if status.is_success() {
                            let text = response.json::<HashMap<String, String>>().await?;
                            // updates the cache content
                            if let Some(location) = text.get("finalized_location") {
                                let id = asset_info.asset_id.clone();
                                let uri = location.to_string();
                                // cache item to update
                                let item = cache.items.0.get_mut(&id).unwrap();
                                match data_type {
                                    DataType::Image => item.image_link = uri,
                                    DataType::Metadata => item.metadata_link = uri,
                                    DataType::Animation => item.animation_link = Some(uri),
                                }
                                // syncs cache (checkpoint)
                                cache.sync_file()?;
                                // updates the progress bar
                                progress.inc(1);
                            }
                        } else {
                            let error = response.json::<HashMap<String, String>>().await?;
                            let message = if let Some(m) = error.get("error") {
                                m.to_string()
                            } else {
                                format!("Error uploading batch (http status {})", status)
                            };
                            errors.push(UploadError::SendDataFailed(message));
                        }
                    } else {
                        errors.push(UploadError::SendDataFailed(
                            "Permission denied (not a file owner)".to_string(),
                        ));
                    }
                } else {
                    errors.push(UploadError::SendDataFailed("Missing owner".to_string()));
                };
            } else {
                let error = response.json::<HashMap<String, String>>().await?;
                let message = if let Some(m) = error.get("error") {
                    m.to_string()
                } else {
                    format!(
                        "Could not retrieve file account data (http status {})",
                        status
                    )
                };

                errors.push(UploadError::SendDataFailed(message));
            }
        }

        Ok(errors)
    }
}

#[async_trait]
impl Prepare for SHDWMethod {
    async fn prepare(
        &self,
        _sugar_config: &SugarConfig,
        assets: &HashMap<isize, AssetPair>,
        asset_indices: Vec<(DataType, &[isize])>,
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

        if self.storage_account.storage < total_size {
            let required = total_size - self.storage_account.storage_available;
            return Err(anyhow!(
                "Insufficient storage space (additional {required} bytes required)"
            ));
        }

        Ok(())
    }
}

#[async_trait]
impl Uploader for SHDWMethod {
    async fn upload(
        &self,
        sugar_config: &SugarConfig,
        cache: &mut Cache,
        data_type: DataType,
        assets: &mut Vec<AssetInfo>,
        progress: &ProgressBar,
        interrupted: Arc<AtomicBool>,
    ) -> Result<Vec<UploadError>> {
        let mut to_upload: Vec<Vec<&AssetInfo>> = Vec::new();
        let mut to_edit: Vec<&AssetInfo> = Vec::new();

        let mut current: Vec<&AssetInfo> = Vec::new();
        let mut tx_size = 0;
        let http_client = reqwest::Client::new();

        for asset_info in assets {
            let location = format!(
                "{}/{}/{}",
                SHDW_DRIVE_LOCATION, &self.storage_pubkey, &asset_info.name,
            );

            let response = http_client.head(&location).send().await?;

            match response.status() {
                StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => {
                    // we will upload the file
                    let size = asset_info.name.as_bytes().len();

                    if current.len() == INSTRUCTION_COUNT_LIMIT
                        || (tx_size + size) > FILE_NAME_LIMIT
                    {
                        to_upload.push(current);
                        current = Vec::new();
                        tx_size = 0;
                    }
                    tx_size += size;
                    current.push(asset_info);
                }
                StatusCode::OK => {
                    // file already exists, we need to submit an edit request
                    to_edit.push(asset_info);
                }
                status => {
                    // could not retrieve the status of the file, will skip it
                    // for now
                    info!(
                        "Skipping upload of file '{}' (http status code {})",
                        asset_info.name, status
                    );
                    println!("Error status: {}", status);
                }
            };
        }
        // adds the last chunk (if there is one)
        if !current.is_empty() {
            to_upload.push(current);
        }

        let mut errors = Vec::new();

        if !to_upload.is_empty() {
            errors.extend(
                self.process_upload(
                    sugar_config,
                    cache,
                    &data_type,
                    &mut to_upload,
                    progress,
                    interrupted.clone(),
                )
                .await?,
            );
        }

        if !to_edit.is_empty() {
            errors.extend(
                self.process_edit(
                    sugar_config,
                    cache,
                    &data_type,
                    &mut to_edit,
                    progress,
                    interrupted.clone(),
                )
                .await?,
            );
        }

        if errors.is_empty() && interrupted.load(Ordering::SeqCst) {
            progress.abandon_with_message(format!("{}", style("Upload aborted ").red().bold()));
            return Err(
                UploadError::SendDataFailed("Not all files were uploaded.".to_string()).into(),
            );
        }

        Ok(errors)
    }
}
