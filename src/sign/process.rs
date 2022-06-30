pub use anchor_client::{
    solana_sdk::{
        commitment_config::{CommitmentConfig, CommitmentLevel},
        native_token::LAMPORTS_PER_SOL,
        pubkey::Pubkey,
        signature::{Keypair, Signature, Signer},
        system_instruction, system_program, sysvar,
        transaction::Transaction,
    },
    Client, Program,
};
use console::style;
use retry::{delay::Exponential, retry};
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use std::{
    io::{stdin, stdout, Write},
    rc::Rc,
    str::FromStr,
    sync::Arc,
};

use mpl_candy_machine::accounts as nft_accounts;
use mpl_candy_machine::instruction as nft_instruction;
use mpl_token_metadata::{instruction::sign_metadata, state::Metadata, ID as METAPLEX_PROGRAM_ID};

use crate::setup::{setup_client, sugar_setup};
use crate::utils::*;
use crate::{cache::load_cache, candy_machine::CANDY_MACHINE_ID};
use crate::{common::*, pdas::get_metadata_pda};

pub struct SignArgs {
    pub candy_machine: Option<String>,
    pub keypair: Option<String>,
    pub cache: String,
    pub rpc_url: Option<String>,
    pub mint: Option<String>,
}

pub fn process_sign(args: SignArgs) -> Result<()> {
    // (1) Setting up connection
    println!(
        "{} {}Initializing connection",
        style("[1/2]").bold().dim(),
        COMPUTER_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let (program, payer, creator) = setup_sign(args.keypair, args.rpc_url)?;

    let candy_machine_id = match args.candy_machine {
        Some(candy_machine_id) => candy_machine_id,
        None => {
            let cache = load_cache(&args.cache, false)?;
            cache.program.candy_machine
        }
    };

    let candy_machine = Pubkey::from_str(&candy_machine_id)?;

    pb.finish_with_message("Connected");

    println!(
        "\n{} {}{}",
        style("[2/2]").bold().dim(),
        SIGNING_EMOJI,
        if args.mint.is_some() {
            "Signing one NFT"
        } else {
            "Signing all NFTs"
        }
    );

    if let Some(mint_id) = args.mint {
        let pb = spinner_with_style();
        pb.set_message(format!("Signing NFT with mint id {}.", mint_id));

        let account_pubkey = Pubkey::from_str(&mint_id)?;
        let metadata_pubkey = get_metadata_pda(&account_pubkey, &program)?;

        let result = match sign(program, &creator, metadata_pubkey.0) {
            Ok(signature) => format!("{} {:?}", style("Signature:").bold(), signature),
            Err(err) => {
                pb.abandon_with_message(format!("{}", style("Signing failed ").red().bold()));
                error!("{:?}", err);
                return Err(err);
            }
        };

        pb.finish_with_message(result);
    }
    // else {
    //     let pb = progress_bar_with_style(number);

    //     for _i in 0..number {
    //         if let Err(err) = mint(
    //             Arc::clone(&client),
    //             candy_pubkey,
    //             Arc::clone(&candy_machine_state),
    //             Arc::clone(&collection_pda_info),
    //         ) {
    //             pb.abandon_with_message(format!("{}", style("Mint failed ").red().bold()));
    //             error!("{:?}", err);
    //             return Err(err);
    //         }

    //         pb.inc(1);
    //     }

    //     pb.finish();
    // }

    Ok(())
}

fn setup_sign(
    keypair: Option<String>,
    rpc_url: Option<String>,
) -> Result<(Program, Pubkey, Keypair)> {
    let sugar_config = sugar_setup(keypair, rpc_url)?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID);
    let payer = program.payer();

    Ok((program, payer, sugar_config.keypair))
}

fn sign(program: Program, creator: &Keypair, metadata: Pubkey) -> Result<Signature> {
    let recent_blockhash = program.rpc().get_latest_blockhash()?;
    let ix = sign_metadata(METAPLEX_PROGRAM_ID, metadata, creator.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&creator.pubkey()),
        &[creator],
        recent_blockhash,
    );

    // Send tx with retries.
    let res = retry(
        Exponential::from_millis_with_factor(250, 2.0).take(3),
        || program.rpc().send_and_confirm_transaction(&tx),
    );
    let sig = res?;

    Ok(sig)
}
