use std::str::FromStr;

use anchor_client::solana_sdk::{compute_budget::ComputeBudgetInstruction, pubkey::Pubkey};
use anyhow::Result;
use console::style;
use mpl_candy_machine_core::{accounts::SetTokenStandard, AccountVersion};
use mpl_token_metadata::{
    instruction::MetadataDelegateRole,
    pda::{find_collection_authority_account, find_metadata_delegate_record_account},
};

use crate::{
    cache::load_cache,
    candy_machine::{get_candy_machine_state, CANDY_MACHINE_ID},
    common::*,
    config::TokenStandard,
    pdas::{find_candy_machine_creator_pda, find_metadata_pda, get_metadata_pda},
    utils::*,
};

pub struct SetTokenStandardArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub token_standard: Option<TokenStandard>,
    pub candy_machine: Option<String>,
    pub rule_set: Option<String>,
    pub priority_fee: u64,
}

pub fn process_set_token_stardard(args: SetTokenStandardArgs) -> Result<()> {
    // validate that we got the required input
    if args.token_standard.is_none() && args.rule_set.is_none() {
        return Err(anyhow!(
            "You need to specify a token standard and/or rule set."
        ));
    }

    println!("[1/2] {}Loading candy machine", LOOKING_GLASS_EMOJI);

    // the candy machine id specified takes precedence over the one from the cache

    let candy_machine_id = if let Some(candy_machine) = args.candy_machine {
        candy_machine
    } else {
        let cache = load_cache(&args.cache, false)?;
        cache.program.candy_machine
    };

    if candy_machine_id.is_empty() {
        return Err(anyhow!("Missing candy guard id."));
    }

    let candy_machine_id = match Pubkey::from_str(&candy_machine_id) {
        Ok(candy_machine_id) => candy_machine_id,
        Err(_) => {
            let error = anyhow!("Failed to parse candy machine id: {}", candy_machine_id);
            error!("{:?}", error);
            return Err(error);
        }
    };

    let sugar_config = sugar_setup(args.keypair, args.rpc_url)?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID);

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let candy_machine_state = get_candy_machine_state(&sugar_config, &candy_machine_id)?;

    pb.finish_with_message("Done");

    let message = if args.token_standard.is_some() {
        if args.rule_set.is_some() {
            "token standard and rule set"
        } else {
            "token standard"
        }
    } else {
        "rule set"
    };

    println!("\n[2/2] {}Setting {}", WITHDRAW_EMOJI, message);

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let (authority_pda, _) = find_candy_machine_creator_pda(&candy_machine_id);
    let collection_mint = candy_machine_state.collection_mint;
    let collection_metadata = find_metadata_pda(&collection_mint);
    let (_, collection_metadata_pda) =
        get_metadata_pda(&candy_machine_state.collection_mint, &program)?;
    let collection_update_authority = collection_metadata_pda.update_authority;

    let collection_authority_record = if matches!(candy_machine_state.version, AccountVersion::V1) {
        Some(find_collection_authority_account(&collection_mint, &authority_pda).0)
    } else {
        None
    };

    let collection_delegate_record = find_metadata_delegate_record_account(
        &collection_mint,
        MetadataDelegateRole::Collection,
        &collection_update_authority,
        &authority_pda,
    )
    .0;

    // either uses the specified token standard or the existing one, for the case
    // where only the rule set will be set
    let token_standard = if let Some(token_standard) = args.token_standard {
        <TokenStandard as std::convert::Into<mpl_token_metadata::state::TokenStandard>>::into(
            token_standard,
        ) as u8
    } else {
        candy_machine_state.token_standard
    };

    let payer = sugar_config.keypair;

    let compute_units = ComputeBudgetInstruction::set_compute_unit_limit(COMPUTE_UNITS);
    let priority_fee = ComputeBudgetInstruction::set_compute_unit_price(args.priority_fee);

    let tx = program
        .request()
        .instruction(compute_units)
        .instruction(priority_fee)
        .accounts(SetTokenStandard {
            candy_machine: candy_machine_id,
            authority_pda,
            authority: payer.pubkey(),
            payer: payer.pubkey(),
            collection_metadata,
            collection_mint,
            collection_update_authority,
            collection_authority_record,
            collection_delegate_record,
            rule_set: if let Some(rule_set) = args.rule_set {
                Some(Pubkey::from_str(&rule_set)?)
            } else {
                None
            },
            system_program: system_program::ID,
            sysvar_instructions: sysvar::instructions::ID,
            token_metadata_program: mpl_token_metadata::ID,
            authorization_rules_program: None,
            authorization_rules: None,
        })
        .args(mpl_candy_machine_core::instruction::SetTokenStandard { token_standard });

    let sig = tx.send()?;

    pb.finish_and_clear();
    println!("{} {}", style("Signature:").bold(), sig);

    Ok(())
}
