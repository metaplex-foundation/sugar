use std::str::FromStr;

use anchor_client::solana_sdk::{compute_budget::ComputeBudgetInstruction, pubkey::Pubkey};
use anyhow::Result;
use console::style;
use mpl_candy_guard::{accounts::Withdraw as WithdrawAccount, instruction::Withdraw};
use solana_program::native_token::LAMPORTS_PER_SOL;

use crate::{cache::load_cache, common::*, utils::*};

pub struct GuardWithdrawArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub candy_guard: Option<String>,
    pub priority_fee: u64,
}

pub fn process_guard_withdraw(args: GuardWithdrawArgs) -> Result<()> {
    println!("[1/2] {LOOKING_GLASS_EMOJI}Loading candy guard");

    // the candy guard id specified takes precedence over the one from the cache

    let (candy_guard_id, cache) = if let Some(candy_guard) = args.candy_guard {
        (candy_guard, None)
    } else {
        let cache = load_cache(&args.cache, false)?;
        (cache.program.candy_guard.clone(), Some(cache))
    };

    if candy_guard_id.is_empty() {
        return Err(anyhow!("Missing candy guard id."));
    }

    let candy_guard_id = match Pubkey::from_str(&candy_guard_id) {
        Ok(candy_guard_id) => candy_guard_id,
        Err(_) => {
            let error = anyhow!("Failed to parse candy guard id: {}", candy_guard_id);
            error!("{:?}", error);
            return Err(error);
        }
    };

    let sugar_config = sugar_setup(args.keypair, args.rpc_url)?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(mpl_candy_guard::ID);
    let payer = sugar_config.keypair;

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let account = program.rpc().get_account(&candy_guard_id)?;

    pb.finish_with_message("Done");

    println!("\n[2/2] {WITHDRAW_EMOJI}Retrieving funds");

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let priority_fee = ComputeBudgetInstruction::set_compute_unit_price(args.priority_fee);

    let tx = program
        .request()
        .instruction(priority_fee)
        .accounts(WithdrawAccount {
            candy_guard: candy_guard_id,
            authority: payer.pubkey(),
        })
        .args(Withdraw {});

    let sig = tx.send()?;

    pb.finish_and_clear();
    println!("{} {}", style("Signature:").bold(), sig);

    println!(
        "\nReceived ◎ {} from rent fee.",
        (account.lamports as f64) / (LAMPORTS_PER_SOL as f64)
    );

    // if we closed the candy guard from the cache file, remove
    // its reference

    if cache.is_some() {
        let mut cache = load_cache(&args.cache, false)?;
        cache.program.candy_guard = String::new();
        cache.sync_file()?;
    }

    Ok(())
}
