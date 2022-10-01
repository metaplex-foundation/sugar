use std::path::Path;

use anyhow::Result;

use crate::{
    common::*,
    mint::airdrop::{
        errors::AirDropError,
        structs::{AirDropResults, AirDropTargets},
    },
};

pub fn load_airdrop_results(airdrop_list: &mut AirDropTargets) -> Result<AirDropResults> {
    let airdrop_results_path = Path::new("airdrop_results.json");
    if !airdrop_results_path.exists() {
        return Ok(AirDropResults::new());
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

    let results: AirDropResults = match serde_json::from_reader(file) {
        Ok(airdrop_results) => airdrop_results,
        Err(err) => {
            return Err(AirDropError::AirDropResultsFileWrongFormat(
                path_to_string(airdrop_results_path)?,
                err.to_string(),
            )
            .into());
        }
    };

    for (address, transactions) in results.iter() {
        if !airdrop_list.contains_key(address) {
            continue;
        }

        let mut target = *airdrop_list.get(address).unwrap();
        for transaction in transactions.iter() {
            if transaction.status {
                // target = &mut target.checked_sub(1).ok_or(AirDropError::OverflowDuringSyncOfResultsAndTargetsForAddress(address.to_string()).into())?;
                target -= 1;
                // airdrop_list.total = airdrop_list.total.checked_sub(1).ok_or(AirDropError::OverflowDuringSyncOfResultsAndTargetsForAddress(address.to_string()).into())?;
                // airdrop_list.total -= 1;
            }
        }
        airdrop_list.insert(*address, target);
    }

    Ok(results)
}

pub fn load_airdrop_list(airdrop_list: String) -> Result<AirDropTargets> {
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

    let targets: AirDropTargets = match serde_json::from_reader(file) {
        Ok(airdrop_list) => airdrop_list,
        Err(err) => {
            return Err(
                AirDropError::AirDropListFileWrongFormat(airdrop_list, err.to_string()).into(),
            );
        }
    };

    Ok(targets)
}
