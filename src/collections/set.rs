use std::{ops::Deref, str::FromStr};

use anchor_client::solana_sdk::{
    compute_budget::ComputeBudgetInstruction, pubkey::Pubkey, system_program,
};
use anyhow::Result;
use console::style;
use mpl_core::accounts::BaseCollectionV1;
use mpl_core_candy_machine_core::{accounts as nft_accounts, instruction as nft_instruction};

use crate::{
    cache::load_cache,
    candy_machine::{CANDY_MACHINE_ID, *},
    common::*,
    config::get_config_data,
    hash::hash_and_update,
    pdas::*,
    update::{process_update, UpdateArgs},
    utils::{assert_correct_authority, get_base_collection, spinner_with_style},
};

pub struct SetCollectionArgs {
    pub collection_mint: String,
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub config: String,
    pub candy_machine: Option<String>,
    pub priority_fee: u64,
}

pub fn process_set_collection(args: SetCollectionArgs) -> Result<()> {
    let sugar_config = sugar_setup(args.keypair.clone(), args.rpc_url.clone())?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID)?;
    let mut cache = Cache::new();

    // The candy machine id specified takes precedence over the one from the cache.
    let candy_machine_id = match args.candy_machine {
        Some(ref candy_machine_id) => candy_machine_id.to_owned(),
        None => {
            cache = load_cache(&args.cache, false)?;
            cache.program.candy_machine.clone()
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

    let collection_info = get_base_collection(&collection_mint_pubkey, &program)?;

    pb.finish_with_message("Done");

    assert_correct_authority(
        &sugar_config.keypair.pubkey(),
        &candy_machine_state.authority,
    )?;

    println!(
        "\n{} {}Setting collection mint for candy machine",
        style("[2/2]").bold().dim(),
        COLLECTION_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Sending set collection transaction...");

    let set_signature = set_collection(
        &program,
        &candy_pubkey,
        &candy_machine_state,
        &collection_mint_pubkey,
        &collection_info,
        &args,
    )?;

    pb.finish_with_message(format!(
        "{} {}",
        style("Set collection signature:").bold(),
        set_signature
    ));

    // If a candy machine id wasn't manually specified we are operating on the candy machine in the cache
    // and so need to update the cache file.
    if args.candy_machine.is_none() {
        cache.items.shift_remove("-1");
        cache.program.collection_mint = collection_mint_pubkey.to_string();
        cache.sync_file()?;

        // If hidden settings are enabled, we update the hash value in the config file and update the candy machine on-chain.
        if candy_machine_state.data.hidden_settings.is_some() {
            let mut config_data = get_config_data(&args.config)?;
            let hidden_settings = config_data.hidden_settings.as_ref().unwrap().clone();

            println!(
                "\n{} {}",
                style("Hidden settings hash:").bold(),
                hash_and_update(hidden_settings, &args.config, &mut config_data, &args.cache,)?
            );

            println!(
                "\nCandy machine has hidden settings and cache file was updated. Updating hash value...\n"
            );

            let update_args = UpdateArgs {
                keypair: args.keypair,
                rpc_url: args.rpc_url,
                cache: args.cache,
                new_authority: None,
                config: args.config,
                candy_machine: Some(candy_machine_id),
                priority_fee: args.priority_fee,
            };

            process_update(update_args)?;
        }
    }

    Ok(())
}

pub fn set_collection<C: Deref<Target = impl Signer> + Clone>(
    program: &Program<C>,
    candy_pubkey: &Pubkey,
    candy_machine_state: &CandyMachine,
    new_collection_mint_pubkey: &Pubkey,
    new_collection: &BaseCollectionV1,
    args: &SetCollectionArgs,
) -> Result<Signature> {
    let payer = program.payer();

    // let (authority_pda, _) = find_candy_machine_creator_pda(candy_pubkey);

    if new_collection.update_authority != payer {
        return Err(anyhow!(CustomCandyError::AuthorityMismatch(
            new_collection.update_authority.to_string(),
            payer.to_string()
        )));
    }

    if candy_machine_state.items_redeemed > 0 {
        return Err(anyhow!(
            "You can't modify the Candy Machine collection after items have been minted."
        ));
    }

    let collection_mint = candy_machine_state.collection_mint;
    let collection = get_base_collection(&candy_machine_state.collection_mint, program)?;

    let (authority_pda, _) = find_candy_machine_creator_pda(candy_pubkey);

    let collection_update_authority = collection.update_authority;

    let priority_fee = ComputeBudgetInstruction::set_compute_unit_price(args.priority_fee);

    let builder = program
        .request()
        .instruction(priority_fee)
        .accounts(nft_accounts::SetCollection {
            candy_machine: *candy_pubkey,
            authority: payer,
            authority_pda,
            payer,
            collection_update_authority,
            collection: collection_mint,
            new_collection_update_authority: new_collection.update_authority,
            new_collection: *new_collection_mint_pubkey,
            mpl_core_program: mpl_core::ID,
            system_program: system_program::ID,
            sysvar_instructions: sysvar::instructions::ID,
        })
        .args(nft_instruction::SetCollection);

    let sig = builder.send()?;

    Ok(sig)
}
