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
use futures::future::select_all;
use mpl_token_metadata::{instruction::sign_metadata, ID as METAPLEX_PROGRAM_ID};
use retry::{delay::Exponential, retry};
use solana_client::rpc_client::RpcClient;
use solana_transaction_crawler::crawler::Crawler;
use std::{
    cmp,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::{
    cache::load_cache,
    candy_machine::CANDY_MACHINE_ID,
    common::*,
    config::SugarConfig,
    pdas::find_metadata_pda,
    setup::{setup_client, sugar_setup},
    utils::*,
};

pub struct SignArgs {
    pub candy_machine_id: Option<String>,
    pub keypair: Option<String>,
    pub cache: String,
    pub rpc_url: Option<String>,
    pub mint: Option<String>,
    pub interrupted: Arc<AtomicBool>,
}

pub async fn process_sign(args: SignArgs) -> Result<()> {
    // (1) Setting up connection
    println!(
        "{} {}Initializing connection",
        if args.mint.is_some() {
            style("[1/2]").bold().dim()
        } else {
            style("[1/3]").bold().dim()
        },
        COMPUTER_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let sugar_config = Arc::new(sugar_setup(args.keypair, args.rpc_url)?);

    let candy_machine_id = match args.candy_machine_id {
        Some(candy_machine_id) => candy_machine_id,
        None => {
            let cache = load_cache(&args.cache, false)?;
            cache.program.candy_machine
        }
    };

    pb.finish_with_message("Connected");

    if let Some(mint_id) = args.mint {
        println!(
            "\n{} {}Signing one NFT",
            style("[2/2]").bold().dim(),
            SIGNING_EMOJI,
        );
        let pb = spinner_with_style();
        pb.set_message(format!("Signing NFT with mint id {}.", mint_id));

        let account_pubkey = Pubkey::from_str(&mint_id)?;
        let metadata_pubkey = find_metadata_pda(&account_pubkey);

        match sign(Arc::clone(&sugar_config), metadata_pubkey).await {
            Ok(signature) => format!("{} {:?}", style("Signature:").bold(), signature),
            Err(err) => {
                pb.abandon_with_message(format!("{}", style("Signing failed ").red().bold()));
                error!("{:?}", err);
                return Err(err);
            }
        };

        pb.finish();
    } else {
        println!(
            "\n{} {}Fetching mint ids",
            style("[2/3]").bold().dim(),
            SIGNING_EMOJI,
        );

        let mut errors = Vec::new();
        let client = RpcClient::new("https://ssc-dao.genesysgo.net");
        let cm_id = Pubkey::from_str(&candy_machine_id).unwrap();

        let crawled_accounts = &Crawler::get_cmv2_mints(client, cm_id).await?["metadata"];
        let mut account_keys = crawled_accounts
            .into_iter()
            .map(|account| Pubkey::from_str(&account).unwrap())
            .collect::<Vec<Pubkey>>();

        pb.finish_with_message(format!("Found {:?} accounts", account_keys.len() as u64));
        println!(
            "\n{} {}Signing mint accounts",
            style("[3/3]").bold().dim(),
            SIGNING_EMOJI
        );

        let pb = progress_bar_with_style(account_keys.len() as u64);
        args.interrupted.store(false, Ordering::SeqCst);

        let mut handles = Vec::new();
        for account in account_keys.drain(0..cmp::min(account_keys.len(), PARALLEL_LIMIT)) {
            let config = sugar_config.clone();
            handles.push(tokio::spawn(async move {
                sign(Arc::clone(&config), account).await
            }));
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

            if !account_keys.is_empty() {
                // if we are half way through, let spawn more transactions
                if (PARALLEL_LIMIT - handles.len()) > (PARALLEL_LIMIT / 2) {
                    for account in
                        account_keys.drain(0..cmp::min(account_keys.len(), PARALLEL_LIMIT / 2))
                    {
                        let config = sugar_config.clone();
                        handles.push(tokio::spawn(
                            async move { sign(config.clone(), account).await },
                        ));
                    }
                }
            }
        }

        if !errors.is_empty() {
            pb.abandon_with_message(format!(
                "{}",
                style("Signing all NFTs failed ").red().bold()
            ));
            return Err(anyhow!(format!("Not all NFTs were signed.")));
        } else if !account_keys.is_empty() {
            pb.abandon_with_message(format!(
                "{}",
                style("Signing all NFTs aborted ").red().bold()
            ));
            return Err(anyhow!(format!("Not all NFTs were signed.")));
        } else {
            pb.finish_with_message(format!(
                "{}",
                style("All NFTs signed successfully.").green().bold()
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
