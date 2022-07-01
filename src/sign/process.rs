pub use anchor_client::{
    solana_sdk::{
        account::Account,
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
use mpl_token_metadata::ID as TOKEN_METADATA_PROGRAM_ID;
use retry::{delay::Exponential, retry};
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use solana_program::borsh::try_from_slice_unchecked;
use std::{
    rc::Rc,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use mpl_token_metadata::{instruction::sign_metadata, state::Metadata, ID as METAPLEX_PROGRAM_ID};

use crate::setup::{setup_client, sugar_setup};
use crate::utils::*;
use crate::{cache::load_cache, candy_machine::CANDY_MACHINE_ID};
use crate::{common::*, pdas::get_metadata_pda};

pub struct SignArgs {
    pub candy_machine_creator: Option<String>,
    pub keypair: Option<String>,
    pub cache: String,
    pub rpc_url: Option<String>,
    pub mint: Option<String>,
    pub position: usize,
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

    let (program, signer) = setup_sign(args.keypair, args.rpc_url)?;

    let program = Rc::new(program);

    let candy_machine_creator = match args.candy_machine_creator {
        Some(candy_machine_creator) => candy_machine_creator,
        None => {
            let cache = load_cache(&args.cache, false)?;
            cache.program.candy_machine_creator
        }
    };

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

        match sign(program, &signer, metadata_pubkey.0) {
            Ok(signature) => format!("{} {:?}", style("Signature:").bold(), signature),
            Err(err) => {
                pb.abandon_with_message(format!("{}", style("Signing failed ").red().bold()));
                error!("{:?}", err);
                return Err(err);
            }
        };

        pb.finish();
    } else {
        let accounts = get_candy_machine_creator_accounts(
            &program.rpc(),
            &candy_machine_creator,
            args.position,
        )?;

        let signed_at_least_one_account = Arc::new(AtomicBool::new(false));
        let pb = progress_bar_with_style(accounts.len() as u64);
        let mut not_signed = 0;

        accounts.iter().for_each(|(metadata_pubkey, account)| {
            let signed_at_least_one_account = signed_at_least_one_account.clone();
            let metadata: Metadata = match try_from_slice_unchecked(&account.data.clone()) {
                Ok(metadata) => metadata,
                Err(_) => {
                    error!("Account {} has no metadata", metadata_pubkey);
                    return;
                }
            };

            if let Some(creators) = metadata.data.creators {
                // Check whether the specific creator has already signed the account
                for creator in creators {
                    if creator.address == signer.pubkey() && !creator.verified {
                        sign(program.clone(), &signer, *metadata_pubkey).unwrap_or_else(|e| {
                            not_signed += 1;
                            error!("Error signing: {}", e);
                        });

                        signed_at_least_one_account.store(true, Ordering::Relaxed);
                        pb.inc(1);
                    }
                }
            }
        });

        if not_signed > 0 {
            println!(
                "{}",
                style(format!("Could not sign {} nft(s)", not_signed))
                    .red()
                    .bold()
                    .dim()
            );
        }

        pb.finish();
    }

    Ok(())
}

fn setup_sign(keypair: Option<String>, rpc_url: Option<String>) -> Result<(Program, Keypair)> {
    let sugar_config = sugar_setup(keypair, rpc_url)?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID);

    Ok((program, sugar_config.keypair))
}

fn sign(program: Rc<Program>, creator: &Keypair, metadata: Pubkey) -> Result<()> {
    let recent_blockhash = program.rpc().get_latest_blockhash()?;
    let ix = sign_metadata(METAPLEX_PROGRAM_ID, metadata, creator.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&creator.pubkey()),
        &[creator],
        recent_blockhash,
    );

    // Send tx with retries.
    retry(
        Exponential::from_millis_with_factor(250, 2.0).take(3),
        || program.rpc().send_and_confirm_transaction(&tx),
    )?;

    Ok(())
}

fn get_candy_machine_creator_accounts(
    client: &RpcClient,
    creator: &str,
    position: usize,
) -> Result<Vec<(Pubkey, Account)>> {
    if position > 4 {
        error!("CM Creator position cannot be greator than 4");
        std::process::exit(1);
    }

    let config = RpcProgramAccountsConfig {
        filters: Some(vec![RpcFilterType::Memcmp(Memcmp {
            offset: 1 + // key
            32 + // update auth
            32 + // mint
            4 + // name string length
            MAX_NAME_LENGTH + // name
            4 + // uri string length
            MAX_URI_LENGTH + // uri*
            4 + // symbol string length
            MAX_SYMBOL_LENGTH + // symbol
            2 + // seller fee basis points
            1 + // whether or not there is a creators vec
            4 + // creators
            position * // index for each creator
            (
                32 + // address
                1 + // verified
                1 // share
            ),
            bytes: MemcmpEncodedBytes::Base58(creator.to_string()),
            encoding: None,
        })]),
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64),
            data_slice: None,
            commitment: Some(CommitmentConfig {
                commitment: CommitmentLevel::Confirmed,
            }),
        },
        with_context: None,
    };

    let accounts = client.get_program_accounts_with_config(&TOKEN_METADATA_PROGRAM_ID, config)?;

    Ok(accounts)
}
