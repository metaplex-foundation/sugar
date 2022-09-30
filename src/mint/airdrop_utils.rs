use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AirDropList {
    pub targets: Vec<AirDropTarget>,
    pub total: u64,
}

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

    #[error("Cannot use number and airdrop feature at the same time")]
    CannotUseNumberAndAirdropFeatureAtTheSameTime,

    #[error("Airdrop total {0} is higher than available {1}")]
    AirdropTotalIsHigherThanAvailable(u64, u64),

    #[error("Failed to open AirDrop results file {0} with error {1}")]
    FailedToOpenAirDropResultsFile(String, String),

    #[error("Failed to parse AirDrop results file {0} with error {1}")]
    AirDropResultsFileWrongFormat(String, String),
}

pub fn load_airdrop_results(airdrop_list: &mut AirDropList) -> Result<Vec<AirDropResult>> {
    let airdrop_results_path = Path::new("airdrop_results.json");
    if !airdrop_results_path.exists() {
        return Ok(vec![]);
    }

    let file = match File::open(airdrop_results_path) {
        Ok(file) => file,
        Err(err) => {
            return Err(AirDropError::FailedToOpenAirDropResultsFile(
                path_to_string(airdrop_results_path)?,
                err.to_string(),
            )
            .into());
        }
    };

    let airdrop_results: Vec<AirDropResult> = match serde_json::from_reader(file) {
        Ok(airdrop_results) => airdrop_results,
        Err(err) => {
            return Err(AirDropError::AirDropResultsFileWrongFormat(
                path_to_string(airdrop_results_path)?,
                err.to_string(),
            )
            .into());
        }
    };

    // TODO: Seems inefficient, might need to refactor as HashMap
    for result in airdrop_results.iter() {
        for target in airdrop_list.targets.iter_mut() {
            if target.address != result.address {
                continue;
            }
            for transaction_result in result.transactions_results.iter() {
                if transaction_result.status {
                    target.num -= 1;
                    airdrop_list.total -= 1;
                }
            }
        }
    }

    Ok(airdrop_results)
}

pub fn load_airdrop_list(airdrop_list: String) -> Result<AirDropList> {
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
    let total = airdrop_list.iter().fold(0, |acc, x| acc + x.num);

    Ok(AirDropList {
        total,
        targets: airdrop_list,
    })
}
