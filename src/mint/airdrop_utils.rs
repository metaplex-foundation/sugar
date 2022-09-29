use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AirDropTarget {
    pub address: Pubkey,
    pub num: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransactionResult {
    pub signature: String,
    pub status: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AirDropResult {
    pub address: Pubkey,
    pub transactions_results: Vec<TransactionResult>,
}

#[derive(Debug, Error)]
pub enum AirDropError {
    #[error("AirDrop lisst file {0} not found")]
    AirDropListFileNotFound(String),

    #[error("Failed to open AirDrop list file {0} with error {1}")]
    FailedToOpenAirDropListFile(String, String),

    #[error("Failed to parse AirDrop list file {0} with error {1}")]
    AirDropListFileWrongFormat(String, String),
}

pub fn load_airdrop_list(airdrop_list: String) -> Result<Vec<AirDropTarget>> {
    let airdrop_list_path = Path::new(&airdrop_list);
    if !airdrop_list_path.exists() {
        return Err(AirDropError::AirDropListFileNotFound(airdrop_list).into());
    }

    let file = match File::open(airdrop_list_path) {
        Ok(file) => file,
        Err(err) => {
            return Err(
                AirDropError::FailedToOpenAirDropListFile(airdrop_list, err.to_string()).into(),
            );
        }
    };

    let airdrop_list: Vec<AirDropTarget> = match serde_json::from_reader(file) {
        Ok(airdrop_list) => airdrop_list,
        Err(err) => {
            return Err(
                AirDropError::AirDropListFileWrongFormat(airdrop_list, err.to_string()).into(),
            );
        }
    };

    Ok(airdrop_list)
}
