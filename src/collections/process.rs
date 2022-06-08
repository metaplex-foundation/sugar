use std::str::FromStr;

use anchor_client::solana_sdk::{pubkey::Pubkey, system_program, sysvar};
use anyhow::Result;
use console::style;
use mpl_candy_machine::instruction as nft_instruction;
use mpl_candy_machine::{accounts as nft_accounts, CandyError};
use mpl_token_metadata::error::MetadataError;
use mpl_token_metadata::pda::find_collection_authority_account;

use crate::cache::load_cache;
use crate::candy_machine::CANDY_MACHINE_ID;
use crate::candy_machine::*;
use crate::common::*;
use crate::pdas::*;
use crate::utils::spinner_with_style;

pub struct SetCollectionArgs {
    pub collection_mint: String,
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub candy_machine: Option<String>,
}
pub struct RemoveCollectionArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub candy_machine: Option<String>,
}

pub fn process_set_collection(args: SetCollectionArgs) -> Result<()> {
    let sugar_config = sugar_setup(args.keypair, args.rpc_url)?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID);
    let payer = program.payer();

    // the candy machine id specified takes precedence over the one from the cache
    let candy_machine_id = match args.candy_machine {
        Some(candy_machine_id) => candy_machine_id,
        None => {
            let cache = load_cache(&args.cache, false)?;
            cache.program.candy_machine
        }
    };

    let collection_mint_pubkey = match Pubkey::from_str(&args.collection_mint) {
        Ok(candy_pubkey) => candy_pubkey,
        Err(_) => {
            let error = anyhow!(
                "Failed to parse collection mint id: {}",
                args.collection_mint
            );
            error!("{:?}", error);
            return Err(error);
        }
    };

    let candy_pubkey = match Pubkey::from_str(&candy_machine_id) {
        Ok(candy_pubkey) => candy_pubkey,
        Err(_) => {
            let error = anyhow!("Failed to parse candy machine id: {}", candy_machine_id);
            error!("{:?}", error);
            return Err(error);
        }
    };

    println!(
        "{} {}Loading candy machine",
        style("[1/2]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );
    println!("{} {}", style("Candy machine ID:").bold(), candy_machine_id);

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let candy_machine_state =
        get_candy_machine_state(&sugar_config, &Pubkey::from_str(&candy_machine_id)?)?;

    let (collection_metadata_pubkey, collection_metadata) =
        get_metadata_pda(&collection_mint_pubkey, &program)?;

    let (collection_edition_pubkey, collection_edition) =
        get_master_edition_pda(&collection_mint_pubkey, &program)?;

    pb.finish_with_message("Done");

    println!(
        "{} {}Setting collection mint for candy machine",
        style("[2/2]").bold().dim(),
        CANDY_EMOJI
    );

    if !candy_machine_state.data.retain_authority {
        return Err(anyhow!(CandyError::CandyCollectionRequiresRetainAuthority));
    }

    if collection_metadata.update_authority != payer {
        return Err(anyhow!(CustomCandyError::AuthorityMismatch(
            collection_metadata.update_authority.to_string(),
            payer.to_string()
        )));
    }

    if collection_edition.max_supply != Some(0) {
        return Err(anyhow!(MetadataError::CollectionMustBeAUniqueMasterEdition));
    }

    if candy_machine_state.items_redeemed > 0 {
        return Err(anyhow!(
            "You can't modify the Candy Machine collection after items have been minted."
        ));
    }

    let collection_pda_pubkey = find_collection_pda(&candy_pubkey).0;

    let collection_authority_record =
        find_collection_authority_account(&collection_mint_pubkey, &collection_pda_pubkey).0;

    let builder = program
        .request()
        .accounts(nft_accounts::SetCollection {
            candy_machine: candy_pubkey,
            authority: payer,
            collection_pda: collection_pda_pubkey,
            payer,
            system_program: system_program::id(),
            rent: sysvar::rent::ID,
            metadata: collection_metadata_pubkey,
            mint: collection_mint_pubkey,
            edition: collection_edition_pubkey,
            collection_authority_record,
            token_metadata_program: mpl_token_metadata::ID,
        })
        .args(nft_instruction::SetCollection);

    let pb = spinner_with_style();
    pb.set_message("Sending set collection transaction...");

    let set_signature = builder.send()?;

    pb.finish_with_message(format!(
        "{} {}",
        style("Set collection signature:").bold(),
        set_signature
    ));

    Ok(())
}

pub fn process_remove_collection(args: RemoveCollectionArgs) -> Result<()> {
    let sugar_config = sugar_setup(args.keypair, args.rpc_url)?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID);
    let payer = program.payer();
    // the candy machine id specified takes precedence over the one from the cache
    let candy_machine_id = match args.candy_machine {
        Some(candy_machine_id) => candy_machine_id,
        None => {
            let cache = load_cache(&args.cache, false)?;
            cache.program.candy_machine
        }
    };

    let candy_pubkey = match Pubkey::from_str(&candy_machine_id) {
        Ok(candy_pubkey) => candy_pubkey,
        Err(_) => {
            let error = anyhow!("Failed to parse candy machine id: {}", candy_machine_id);
            error!("{:?}", error);
            return Err(error);
        }
    };

    println!(
        "{} {}Loading candy machine",
        style("[1/2]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );
    println!("{} {}", style("Candy machine ID:").bold(), candy_machine_id);

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let candy_machine_state = get_candy_machine_state(&sugar_config, &candy_pubkey)?;
    let (collection_pda_pubkey, collection_pda) = get_collection_pda(&candy_pubkey, &program)?;
    let collection_mint_pubkey = collection_pda.mint;
    let (collection_metadata_pubkey, collection_metadata) =
        get_metadata_pda(&collection_mint_pubkey, &program)?;

    pb.finish_with_message("Done");

    println!(
        "{} {}Removing collection mint for candy machine",
        style("[2/2]").bold().dim(),
        CANDY_EMOJI
    );

    if collection_metadata.update_authority != payer {
        return Err(anyhow!(CustomCandyError::AuthorityMismatch(
            collection_metadata.update_authority.to_string(),
            payer.to_string()
        )));
    }

    if candy_machine_state.items_redeemed > 0 {
        return Err(anyhow!(
            "You can't modify the Candy Machine collection after items have been minted."
        ));
    }

    let collection_authority_record =
        find_collection_authority_account(&collection_mint_pubkey, &collection_pda_pubkey).0;

    let builder = program
        .request()
        .accounts(nft_accounts::RemoveCollection {
            candy_machine: candy_pubkey,
            authority: payer,
            collection_pda: collection_pda_pubkey,
            metadata: collection_metadata_pubkey,
            mint: collection_mint_pubkey,
            collection_authority_record,
            token_metadata_program: mpl_token_metadata::ID,
        })
        .args(nft_instruction::RemoveCollection);

    let pb = spinner_with_style();
    pb.set_message("Sending remove collection transaction...");

    let remove_signature = builder.send()?;

    pb.finish_with_message(format!(
        "{} {}",
        style("Remove collection signature:").bold(),
        remove_signature
    ));

    Ok(())
}
