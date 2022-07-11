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

use anchor_lang::accounts;
use console::style;
use futures::future::select_all;
use mpl_token_metadata::{instruction::sign_metadata, ID as TOKEN_METADATA_PROGRAM_ID};
use retry::{delay::Exponential, retry};
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use solana_program::borsh::try_from_slice_unchecked;
use std::{
    cmp,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use mpl_token_metadata::{state::Metadata, ID as METAPLEX_PROGRAM_ID};

use crate::utils::*;
use crate::{cache::load_cache, candy_machine::CANDY_MACHINE_ID};
use crate::{common::*, pdas::get_metadata_pda};
use crate::{
    config::SugarConfig,
    setup::{setup_client, sugar_setup},
};

pub struct SignArgs {
    pub candy_machine_creator: Option<String>,
    pub keypair: Option<String>,
    pub cache: String,
    pub rpc_url: Option<String>,
    pub mint: Option<String>,
    pub position: usize,
    pub interrupted: Arc<AtomicBool>,
}

pub async fn process_sign(args: SignArgs) -> Result<()> {
    // (1) Setting up connection
    println!(
        "{} {}Initializing connection",
        style("[1/2]").bold().dim(),
        COMPUTER_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let sugar_config = Arc::new(sugar_setup(args.keypair, args.rpc_url)?);

    // let commitment = CommitmentConfig::from_str(&commitment)?;
    // let timeout = Duration::from_secs(options.timeout);

    // let client = RpcClient::new_with_timeout_and_commitment(rpc.clone(), timeout, commitment);
    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID);

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

        match sign(Arc::clone(&sugar_config), metadata_pubkey.0).await {
            Ok(signature) => format!("{} {:?}", style("Signature:").bold(), signature),
            Err(err) => {
                pb.abandon_with_message(format!("{}", style("Signing failed ").red().bold()));
                error!("{:?}", err);
                return Err(err);
            }
        };

        pb.finish();
    } else {
        let pb = spinner_with_style();
        pb.set_message("Fetching candy machine mint ids...");

        let mut accounts = get_candy_machine_creator_accounts(
            &program.rpc(),
            &candy_machine_creator,
            args.position,
        )
        .await?;

        pb.finish_with_message("Done");

        let pb = progress_bar_with_style(accounts.len() as u64);
        let mut handles = Vec::new();

        let mut errors = Vec::new();

        for tx in accounts.drain(0..cmp::min(accounts.len(), PARALLEL_LIMIT)) {
            let metadata: Metadata = match try_from_slice_unchecked(&tx.1.data.clone()) {
                Ok(metadata) => metadata,
                Err(_) => return Err(anyhow!("Account {} has no metadata", tx.0)),
            };

            if let Some(creators) = metadata.data.creators {
                // Check whether the specific creator has already signed the account
                for creator in creators {
                    let config = sugar_config.clone();
                    if creator.address == config.keypair.pubkey() && !creator.verified {
                        println!("here");
                        handles.push(tokio::spawn(async move {
                            sign(Arc::clone(&config), tx.0).await
                        }));
                    }
                }
            }
        }

        while !args.interrupted.load(Ordering::SeqCst) && !handles.is_empty() {
            match select_all(handles).await {
                (Ok(res), _index, remaining) => {
                    // independently if the upload was successful or not
                    // we continue to try the remaining ones
                    handles = remaining;

                    if res.is_ok() {
                        // updates the progress bar
                        pb.inc(1);
                    } else {
                        // user will need to retry the upload
                        errors.push(anyhow!(format!(
                            "Transaction error: {:?}",
                            res.err().unwrap()
                        )));
                    }
                }
                (Err(err), _index, remaining) => {
                    // user will need to retry the upload
                    errors.push(anyhow!(format!("Transaction error: {:?}", err)));
                    // ignoring all errors
                    handles = remaining;
                }
            }

            if !accounts.is_empty() {
                // if we are half way through, let spawn more transactions
                if (PARALLEL_LIMIT - handles.len()) > (PARALLEL_LIMIT / 2) {
                    for tx in accounts.drain(0..cmp::min(accounts.len(), PARALLEL_LIMIT / 2)) {
                        let config = sugar_config.clone();
                        handles.push(tokio::spawn(async move {
                            sign(Arc::clone(&config), tx.0).await
                        }));
                    }
                }
            }
        }

        if !errors.is_empty() {
            pb.abandon_with_message(format!(
                "{}",
                style("Signing all NFTs failed ").red().bold()
            ));
        } else if !accounts.is_empty() {
            pb.abandon_with_message(format!(
                "{}",
                style("Signing all NFTs aborted ").red().bold()
            ));
            return Err(anyhow!(format!("Not all NFTs were signed.")));
        } else {
            pb.finish_with_message(format!(
                "{}",
                style("Write config lines successful ").green().bold()
            ));
        }
    }

    Ok(())
}

async fn sign(config: Arc<SugarConfig>, metadata: Pubkey) -> Result<()> {
    let client = setup_client(&config)?;
    let program = client.program(CANDY_MACHINE_ID);

    let recent_blockhash = program.rpc().get_latest_blockhash()?;
    let ix = sign_metadata(METAPLEX_PROGRAM_ID, metadata, config.keypair.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&config.keypair.pubkey()),
        &[&config.keypair],
        recent_blockhash,
    );

    // Send tx with retries.
    retry(
        Exponential::from_millis_with_factor(250, 2.0).take(3),
        || program.rpc().send_and_confirm_transaction(&tx),
    )?;

    Ok(())
}

async fn get_candy_machine_creator_accounts(
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
            // min_context_slot: None,
        },
        with_context: None,
    };

    let accounts = client.get_program_accounts_with_config(&TOKEN_METADATA_PROGRAM_ID, config)?;

    Ok(accounts)
}
