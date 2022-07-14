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
use anyhow::Error;
use console::style;
use futures::future::select_all;

use mpl_token_metadata::instruction::sign_metadata;
use retry::{delay::Exponential, retry};
use solana_client::{
    rpc_client::GetConfirmedSignaturesForAddress2Config,
    rpc_response::RpcConfirmedTransactionStatusWithSignature,
};
use solana_program::borsh::try_from_slice_unchecked;
use solana_transaction_status::{EncodedConfirmedTransaction, UiTransactionEncoding};
use std::{
    cmp,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use mpl_token_metadata::{state::Metadata, ID as METAPLEX_PROGRAM_ID};

use crate::{cache::load_cache, candy_machine::CANDY_MACHINE_ID};
use crate::{common::*, pdas::find_metadata_pda, utils::*};
use crate::{
    config::SugarConfig,
    setup::{setup_client, sugar_setup},
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
        style("[1/2]").bold().dim(),
        COMPUTER_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let sugar_config = Arc::new(sugar_setup(args.keypair, args.rpc_url)?);

    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID);

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
            style("[2/4]").bold().dim(),
            SIGNING_EMOJI,
        );

        let mut errors = Vec::new();
        let pb = spinner_with_style();
        pb.set_message("Fetching...");

        let (mints, mint_errors) =
            get_candy_machine_mints(&program, sugar_config.clone(), candy_machine_id).await?;

        if !mint_errors.is_empty() {
            pb.finish_with_message(format!("{} There were some errors fetching mint ids. Please rerun after all successful signings.",WARNING_EMOJI ));
        } else {
            pb.finish_with_message("Done");
        }

        println!(
            "\n{} {}Fetching mint accounts",
            style("[3/4]").bold().dim(),
            SIGNING_EMOJI,
        );

        let pb = spinner_with_style();
        pb.set_message("Fetching...");

        let mut accounts = fetch_accounts(sugar_config.clone(), mints).await?;

        pb.finish_with_message(format!(
            "Found {:?} unsigned accounts",
            accounts.len() as u64
        ));

        println!(
            "\n{} {}Signing mint accounts",
            style("[4/4]").bold().dim(),
            SIGNING_EMOJI
        );

        let pb = progress_bar_with_style(accounts.len() as u64);
        args.interrupted.store(false, Ordering::SeqCst);

        let mut handles = Vec::new();
        for tx in accounts.drain(0..cmp::min(accounts.len(), 60)) {
            let config = sugar_config.clone();
            handles.push(tokio::spawn(
                async move { sign(Arc::clone(&config), *tx).await },
            ));
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
                        handles.push(tokio::spawn(async move { sign(config.clone(), *tx).await }));
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

async fn get_candy_machine_mints(
    client: &Program,
    config: Arc<SugarConfig>,
    candy_machine_id: String,
) -> Result<(HashMap<Pubkey, Pubkey>, Vec<Error>)> {
    let mut all_signatures = Vec::new();
    let mut retries = 0;
    let mut errors = Vec::new();
    let mut commitment_config = GetConfirmedSignaturesForAddress2Config {
        before: None,
        limit: Some(1000),
        until: None,
        commitment: None,
    };

    loop {
        let commitment_config_ref = GetConfirmedSignaturesForAddress2Config {
            before: commitment_config.before,
            limit: Some(1000),
            until: None,
            commitment: None,
        };

        let signatures = client.rpc().get_signatures_for_address_with_config(
            &Pubkey::from_str(&candy_machine_id).unwrap(),
            commitment_config_ref,
        )?;

        if signatures.is_empty() {
            if retries < 10 {
                retries += 1;
            } else {
                break;
            }
        } else {
            commitment_config.before =
                Some(Signature::from_str(&signatures[signatures.len() - 1].signature).unwrap());
            all_signatures.push(signatures);
            retries = 0;
        }
    }

    let mut flattened = all_signatures
        .into_iter()
        .flatten()
        .collect::<Vec<RpcConfirmedTransactionStatusWithSignature>>();

    let mut handles = Vec::new();
    for tx in flattened.drain(0..cmp::min(flattened.len(), PARALLEL_LIMIT)) {
        let config = Arc::clone(&config);
        handles.push(tokio::spawn(
            async move { get_transaction(config, tx).await },
        ));
    }

    let mut mints_and_metadata = HashMap::new();
    while !handles.is_empty() {
        match select_all(handles).await {
            (Ok(res), _index, remaining) => {
                // independently if the upload was successful or not
                // we continue to try the remaining ones
                handles = remaining;

                if res.is_ok() {
                    let res_ref = &res?;
                    if let Some(transaction) = &(*res_ref).transaction.transaction.decode() {
                        let account_keys = &transaction.message.account_keys;
                        let found = transaction.message.instructions.iter().find(|ix| {
                            let program_id = account_keys[ix.program_id_index as usize];

                            program_id
                                == Pubkey::from_str("cndy3Z4yapfJBmL3ShUp5exZKqR3z33thTzeNMm2gRZ")
                                    .unwrap()
                        });

                        let trx_err = if let Some(meta) = &(*res_ref).transaction.meta {
                            meta.err.is_some()
                        } else {
                            false
                        };

                        let bot_tax = if let Some(meta) = &res_ref.transaction.meta {
                            if let Some(log_messages) = &meta.log_messages {
                                log_messages.iter().find(|log_msg| {
                                    log_msg.contains("Candy Machine Botting is taxed at")
                                })
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        if found.is_some() && !trx_err && bot_tax.is_none() {
                            if found.unwrap().program_id_index >= 16 {
                                let metadata_account = transaction.message.account_keys
                                    [found.unwrap().accounts[4] as usize];
                                let mint = transaction.message.account_keys
                                    [found.unwrap().accounts[5] as usize];

                                mints_and_metadata.insert(mint, metadata_account);
                            }
                        };
                        Some(transaction)
                    } else {
                        None
                    };
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

        if !flattened.is_empty() {
            // if we are half way through, let spawn more transactions
            if (PARALLEL_LIMIT - handles.len()) > (PARALLEL_LIMIT / 2) {
                for tx in flattened.drain(0..cmp::min(flattened.len(), PARALLEL_LIMIT / 2)) {
                    let config = Arc::clone(&config);
                    handles.push(tokio::spawn(
                        async move { get_transaction(config, tx).await },
                    ));
                }
            }
        }
    }

    Ok((mints_and_metadata, errors))
}

async fn fetch_accounts(
    config: Arc<SugarConfig>,
    mints_and_metadata: HashMap<Pubkey, Pubkey>,
) -> Result<Vec<Arc<Pubkey>>> {
    let mut handles = Vec::new();

    let pdas: Vec<Pubkey> = mints_and_metadata.into_values().collect();

    let mut all_pdas = Vec::new();
    let mut i = 0;
    while i < pdas.len() {
        if i + 100 > pdas.len() {
            all_pdas.push(pdas[(i..pdas.len())].to_vec());
        } else {
            all_pdas.push(pdas[(i..i + 100)].to_vec());
        };
        i += 100
    }

    for mint in all_pdas.drain(0..cmp::min(all_pdas.len(), PARALLEL_LIMIT)) {
        let config = Arc::clone(&config);
        handles.push(tokio::spawn(
            async move { get_accounts(config, &mint).await },
        ));
    }

    let mut errors = Vec::new();
    let mut accounts = Vec::new();
    while !handles.is_empty() {
        match select_all(handles).await {
            (Ok(res), _index, remaining) => {
                // independently if the upload was successful or not
                // we continue to try the remaining ones
                handles = remaining;

                if res.is_ok() {
                    for account in res? {
                        if let Some(account) = account {
                            let metadata: Option<Metadata> = if let Ok(metadata) =
                                try_from_slice_unchecked(&account.data.clone())
                            {
                                Some(metadata)
                            } else {
                                None
                            };

                            if let Some(meta) = metadata {
                                if let Some(creators) = meta.data.creators {
                                    for creator in creators {
                                        let config = Arc::clone(&config);
                                        if creator.address == config.keypair.pubkey()
                                            && !creator.verified
                                        {
                                            let mint = &mints_and_metadata;
                                            if let Some(metadata) =
                                                mints_and_metadata.get(&meta.mint)
                                            {
                                                let metadata_ref = Arc::new(*metadata);
                                                accounts.push(metadata_ref)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
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

        if !all_pdas.is_empty() {
            // if we are half way through, let spawn more transactions
            if (PARALLEL_LIMIT - handles.len()) > (PARALLEL_LIMIT / 2) {
                for tx in all_pdas.drain(0..cmp::min(all_pdas.len(), PARALLEL_LIMIT / 2)) {
                    let config = Arc::clone(&config);
                    handles.push(tokio::spawn(async move { get_accounts(config, &tx).await }));
                }
            }
        }
    }

    Ok(accounts)
}

async fn get_transaction(
    config: Arc<SugarConfig>,
    tx: RpcConfirmedTransactionStatusWithSignature,
) -> Result<EncodedConfirmedTransaction> {
    let client = setup_client(&config).unwrap();
    let program = client.program(CANDY_MACHINE_ID);
    let sig = &Signature::from_str(&tx.signature).unwrap();

    let transaction = program
        .rpc()
        .get_transaction(sig, UiTransactionEncoding::Base58)?;

    Ok(transaction)
}

async fn get_accounts(
    config: Arc<SugarConfig>,
    mints: &Vec<Pubkey>,
) -> Result<Vec<Option<Account>>> {
    let client = setup_client(&config).unwrap();
    let program = client.program(CANDY_MACHINE_ID);

    let transaction = program.rpc().get_multiple_accounts(&mints)?;

    Ok(transaction)
}
