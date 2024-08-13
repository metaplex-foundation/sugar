#![allow(clippy::too_many_arguments)]

use std::ops::Deref;

use anchor_client::solana_sdk::compute_budget::ComputeBudgetInstruction;
use mpl_core_candy_guard::{
    accounts::Route as RouteAccount, guards::FreezeInstruction, instruction::Route,
    instructions::RouteArgs, state::GuardType,
};

use super::*;

pub struct InitializeArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub config: String,
    pub candy_guard: Option<String>,
    pub candy_machine: Option<String>,
    pub label: Option<String>,
    pub period: u64,
    pub priority_fee: u64,
}

pub fn process_initialize(args: InitializeArgs) -> Result<()> {
    let sugar_config = sugar_setup(args.keypair.clone(), args.rpc_url.clone())?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(mpl_core_candy_guard::ID)?;

    // candy guard id specified takes precedence over the one from the cache
    let candy_guard_id = match args.candy_guard {
        Some(ref candy_guard_id) => candy_guard_id.to_owned(),
        None => {
            let cache = load_cache(&args.cache, false)?;
            cache.program.candy_guard
        }
    };

    // candy machine id specified takes precedence over the one from the cache
    let candy_machine_id = match args.candy_machine {
        Some(ref candy_machine_id) => candy_machine_id.to_owned(),
        None => {
            let cache = load_cache(&args.cache, false)?;
            cache.program.candy_machine
        }
    };

    let candy_guard = Pubkey::from_str(&candy_guard_id)
        .map_err(|_| anyhow!("Failed to parse candy guard id: {}", &candy_guard_id))?;

    let candy_machine = Pubkey::from_str(&candy_machine_id)
        .map_err(|_| anyhow!("Failed to parse candy machine id: {}", &candy_guard_id))?;

    println!(
        "{} {}Loading freeze guard information",
        style("[1/2]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    // destination address specified takes precedence over the one from the cache
    let (destination_address, mint) = get_destination(
        &program,
        &candy_guard,
        get_config_data(&args.config)?,
        &args.label,
    )?;

    pb.finish_with_message("Done");

    println!(
        "\n{} {}Initializing freeze escrow",
        style("[2/2]").bold().dim(),
        MONEY_BAG_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Sending initialize transaction...");

    let signature = initialize(
        &program,
        &candy_guard,
        &candy_machine,
        &destination_address,
        &args.label,
        args.period,
        mint,
        args.priority_fee,
    )?;

    pb.finish_with_message(format!("{} {}", style("Signature:").bold(), signature));

    Ok(())
}

pub fn initialize<C: Deref<Target = impl Signer> + Clone>(
    program: &Program<C>,
    candy_guard_id: &Pubkey,
    candy_machine_id: &Pubkey,
    destination: &Pubkey,
    label: &Option<String>,
    period: u64,
    mint: Option<Pubkey>,
    priority_fee: u64,
) -> Result<Signature> {
    let mut remaining_accounts = Vec::with_capacity(4);
    let (freeze_pda, _) = find_freeze_pda(candy_guard_id, candy_machine_id, destination);
    remaining_accounts.push(AccountMeta {
        pubkey: freeze_pda,
        is_signer: false,
        is_writable: true,
    });
    remaining_accounts.push(AccountMeta {
        pubkey: program.payer(),
        is_signer: true,
        is_writable: false,
    });
    remaining_accounts.push(AccountMeta {
        pubkey: system_program::id(),
        is_signer: false,
        is_writable: false,
    });

    let freeze_guard = if let Some(mint) = mint {
        remaining_accounts.push(AccountMeta {
            pubkey: get_associated_token_address(&freeze_pda, &mint),
            is_signer: false,
            is_writable: true,
        });
        remaining_accounts.push(AccountMeta {
            pubkey: mint,
            is_signer: false,
            is_writable: false,
        });
        remaining_accounts.push(AccountMeta {
            pubkey: spl_token::ID,
            is_signer: false,
            is_writable: false,
        });
        remaining_accounts.push(AccountMeta {
            pubkey: spl_associated_token_account::ID,
            is_signer: false,
            is_writable: false,
        });
        remaining_accounts.push(AccountMeta {
            pubkey: destination.to_owned(),
            is_signer: false,
            is_writable: false,
        });

        GuardType::FreezeTokenPayment
    } else {
        GuardType::FreezeSolPayment
    };

    let mut data = vec![FreezeInstruction::Initialize as u8];
    data.extend_from_slice(&period.to_le_bytes());

    let priority_fee_ix = ComputeBudgetInstruction::set_compute_unit_price(priority_fee);

    let builder = program
        .request()
        .instruction(priority_fee_ix)
        .accounts(RouteAccount {
            candy_guard: *candy_guard_id,
            candy_machine: *candy_machine_id,
            payer: program.payer(),
        })
        .accounts(remaining_accounts)
        .args(Route {
            args: RouteArgs {
                data,
                guard: freeze_guard,
            },
            label: label.to_owned(),
        });
    let sig = builder.send()?;

    Ok(sig)
}
