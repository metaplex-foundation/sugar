use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anchor_client::solana_sdk::account::Account;
use anchor_lang::AnchorDeserialize;
use console::style;
use futures::future::join_all;
use mpl_core::{
    accounts::BaseAssetV1,
    instructions::{UpdateV1, UpdateV1InstructionArgs},
};
use serde::Serialize;
use solana_client::{client_error::ClientError, rpc_client::RpcClient};
use tokio::sync::Semaphore;

use crate::{
    cache::load_cache,
    candy_machine::{get_candy_machine_state, CANDY_MACHINE_ID},
    common::*,
    config::{get_config_data, Cluster},
    setup::get_rpc_url,
    utils::*,
};

pub struct RevealArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub config: String,
    pub timeout: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct AssetUpdateValues {
    pub asset_pubkey: Pubkey,
    pub asset: BaseAssetV1,
    pub new_uri: String,
    pub new_name: String,
    pub index: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RevealTx {
    asset_pubkey: Pubkey,
    result: RevealResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
enum RevealResult {
    Success,
    Failure(String),
}

// Timeout for the GPA call (in seconds).
const DEFAULT_TIMEOUT: u64 = 300;

pub async fn process_reveal(args: RevealArgs) -> Result<()> {
    println!(
        "{} {}Loading items from the cache",
        style("[1/4]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );

    let spinner = spinner_with_style();
    spinner.set_message("Connecting...");

    let config = get_config_data(&args.config)?;

    // If it's not a Hidden Settings mint, return an error.
    let hidden_settings = if let Some(settings) = config.hidden_settings {
        settings
    } else {
        return Err(anyhow!("Candy machine is not a Hidden Settings mint."));
    };

    let cache = load_cache(&args.cache, false)?;
    let sugar_config = sugar_setup(args.keypair, args.rpc_url.clone())?;
    let anchor_client = setup_client(&sugar_config)?;
    let program = anchor_client.program(CANDY_MACHINE_ID)?;

    let candy_machine_id = match Pubkey::from_str(&cache.program.candy_machine) {
        Ok(candy_machine_id) => candy_machine_id,
        Err(_) => {
            let error = anyhow!(
                "Failed to parse candy machine id: {}",
                &cache.program.candy_machine
            );
            error!("{:?}", error);
            return Err(error);
        }
    };

    spinner.finish_with_message("Done");

    println!(
        "\n{} {}Getting minted NFTs for candy machine {}",
        style("[2/4]").bold().dim(),
        LOOKING_GLASS_EMOJI,
        candy_machine_id
    );

    let spinner = spinner_with_style();
    spinner.set_message("Loading...");
    let solana_cluster: Cluster = get_cluster(program.rpc())?;
    let rpc_url = get_rpc_url(args.rpc_url);

    let solana_cluster = if rpc_url.ends_with("8899") {
        Cluster::Localnet
    } else {
        solana_cluster
    };

    let candy_machine = get_candy_machine_state(&sugar_config, &candy_machine_id)?;

    let metadata_pubkeys = match solana_cluster {
        Cluster::Mainnet | Cluster::Devnet | Cluster::Localnet => {
            let client = RpcClient::new_with_timeout(
                &rpc_url,
                Duration::from_secs(if let Some(timeout) = args.timeout {
                    timeout
                } else {
                    DEFAULT_TIMEOUT
                }),
            );
            get_cm_mint_accounts(&client, &candy_machine.collection_mint.to_string())?
        }
        _ => {
            return Err(anyhow!(
                "Cluster being used is unsupported for this command."
            ))
        }
    };

    if metadata_pubkeys.is_empty() {
        spinner.finish_with_message(format!(
            "{}{:?}",
            style("No NFTs found on ").red().bold(),
            style(solana_cluster).red().bold()
        ));
        return Err(anyhow!(
            "No minted NFTs found for candy machine {}",
            candy_machine_id
        ));
    }

    spinner.finish_with_message(format!(
        "Found {:?} accounts",
        metadata_pubkeys.len() as u64
    ));

    println!(
        "\n{} {}Matching NFTs to cache values",
        style("[3/4]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );
    let spinner = spinner_with_style();

    let mut futures = Vec::new();
    let client = RpcClient::new(&rpc_url);
    let client = Arc::new(client);

    // Get all metadata accounts.
    metadata_pubkeys.as_slice().chunks(100).for_each(|chunk| {
        let client = client.clone();
        futures.push(async move {
            let accounts = async_get_multiple_accounts(client, chunk).await.unwrap();
            Ok::<_, anyhow::Error>(chunk.iter().zip(accounts).collect::<Vec<_>>())
        });
    });
    let results = join_all(futures).await;
    let mut accounts = Vec::new();

    for result in results {
        let res = result
            .unwrap()
            .into_iter()
            .map(|(p, a)| (p, a.unwrap()))
            .collect::<Vec<_>>();
        accounts.extend(res);
    }

    let assets: Vec<(&Pubkey, BaseAssetV1)> = accounts
        .into_iter()
        .map(|a| (a.0, a.1.data))
        .map(|(p, d)| (p, BaseAssetV1::deserialize(&mut d.as_slice()).unwrap()))
        .collect();

    let patterns: Vec<&str> = hidden_settings.name.split('$').collect();
    let index_pattern = patterns
        .get(1)
        .expect("No name pattern set in hidden settings.");

    // Parse the pattern in the hidden settings name to see if NFT numbers are zero or one indexed.
    let index = match *index_pattern {
        "ID" => 0,
        "ID+1" => 1,
        _ => panic!("Invalid name pattern set in hidden settings."),
    };

    // Convert cache to make keys match NFT numbers.
    let nft_lookup: HashMap<String, &CacheItem> = cache
        .items
        .iter()
        .filter(|(k, _)| *k != "-1") // skip collection index
        .filter(|(_, i)| !i.on_chain) // skip already revealed items
        .map(|(k, item)| (increment_key(k, index), item)) // Use the index pattern to increment the key.
        .collect();

    serde_json::to_writer_pretty(File::create("temp.json")?, &nft_lookup)?;

    spinner.finish_with_message("Done");

    let mut update_values = Vec::new();

    println!(
        "\n{} {}Updating NFT URIs from cache values",
        style("[4/4]").bold().dim(),
        UPLOAD_EMOJI
    );

    let name_prefix_pattern = patterns.first().unwrap_or(&"");
    let name_suffix_pattern = patterns.get(2).unwrap_or(&"");

    let pattern = regex::Regex::new(&format!(
        "{}([0-9]+){}",
        name_prefix_pattern, name_suffix_pattern
    ))
    .expect("Failed to create regex pattern.");

    let spinner = spinner_with_style();
    spinner.set_message("Setting up transactions...");
    for a in assets {
        let name = a.clone().1.name;
        let num = match pattern.captures(&name).map(|c| c[1].to_string()) {
            Some(num) => num,
            None => {
                println!(
                    "{}",
                    &format!(
                        "{}{}{}",
                        style("Failed to parse name: ").yellow().bold(),
                        name,
                        style("\nIt may have already been updated").yellow().bold(),
                    )
                );
                println!();
                continue;
            }
        };

        let new_uri = nft_lookup
            .get(&num)
            .filter(|i| !i.on_chain)
            .ok_or_else(|| anyhow!("No URI found for number: {num}"))?
            .metadata_link
            .clone();
        let new_name = nft_lookup
            .get(&num)
            .filter(|i| !i.on_chain)
            .ok_or_else(|| anyhow!("No name found for number: {num}"))?
            .name
            .clone();

        update_values.push(AssetUpdateValues {
            asset_pubkey: *a.0,
            asset: a.1,
            new_uri,
            new_name,
            index: num,
        });
    }
    spinner.finish_and_clear();

    let keypair = Arc::new(sugar_config.keypair);
    let sem = Arc::new(Semaphore::new(1000));
    let reveal_results = Arc::new(Mutex::new(Vec::new()));
    let mut tx_tasks = Vec::new();

    let pb = progress_bar_with_style(metadata_pubkeys.len() as u64);
    pb.set_message("Updating NFTs... ");

    let cache = Arc::new(Mutex::new(cache));

    for item in update_values {
        let permit = Arc::clone(&sem).acquire_owned().await.unwrap();
        let client = client.clone();
        let keypair = keypair.clone();
        let reveal_results = reveal_results.clone();
        let pb = pb.clone();

        let cache = cache.clone();
        let index = item.index.clone();

        tx_tasks.push(tokio::spawn(async move {
            // Move permit into the closure so it is dropped when the task is dropped.
            let _permit = permit;
            let asset_pubkey = item.asset_pubkey;
            let mut tx = RevealTx {
                asset_pubkey,
                result: RevealResult::Success,
            };

            match update_metadata_value(client, keypair, item).await {
                Ok(_) => {
                    let mut cache_mutex = cache.lock().unwrap();
                    let v = cache_mutex.items.get_mut(&index).unwrap();
                    v.on_chain = true;
                    reveal_results.lock().unwrap().push(tx);
                }
                Err(e) => {
                    tx.result = RevealResult::Failure(e.to_string());
                    reveal_results.lock().unwrap().push(tx);
                }
            }

            pb.inc(1);
        }));
    }

    for task in tx_tasks {
        task.await.unwrap();
    }
    pb.finish();

    let results = reveal_results.lock().unwrap();

    let errors: Vec<&RevealTx> = results
        .iter()
        .filter(|r| matches!(r.result, RevealResult::Failure(_)))
        .collect();

    if !errors.is_empty() {
        println!(
            "{}Some reveals failed. See the reveal cache file for details. Re-run the command.",
            WARNING_EMOJI
        );
        let f = File::create("sugar-reveal-cache.json")
            .map_err(|e| anyhow!("Failed to create sugar reveal cache file: {e}"))?;
        serde_json::to_writer_pretty(f, &errors).unwrap();
    } else {
        println!("\n{}Reveal complete!", CONFETTI_EMOJI);
    }

    Ok(())
}

async fn async_get_multiple_accounts(
    client: Arc<RpcClient>,
    pubkeys: &[Pubkey],
) -> Result<Vec<Option<Account>>, ClientError> {
    client.get_multiple_accounts(pubkeys)
}

async fn update_metadata_value(
    client: Arc<RpcClient>,
    update_authority: Arc<Keypair>,
    value: AssetUpdateValues,
) -> Result<(), ClientError> {
    let mut data = value.asset.clone();
    if data.uri.trim_matches(char::from(0)) != value.new_uri.trim_matches(char::from(0)) {
        data.uri.clone_from(&value.new_uri);
        data.name.clone_from(&value.new_name);

        let update_authority_address = match value.clone().asset.update_authority {
            mpl_core::types::UpdateAuthority::Collection(address) => Some(address),
            _ => unreachable!(),
        };

        let ix = UpdateV1 {
            asset: value.asset_pubkey,
            collection: update_authority_address,
            payer: update_authority.pubkey(),
            authority: None,
            system_program: system_program::ID,
            log_wrapper: None,
        }
        .instruction(UpdateV1InstructionArgs {
            new_name: Some(value.new_name),
            new_uri: Some(value.new_uri),
            new_update_authority: None,
        });

        let recent_blockhash = client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&update_authority.pubkey()),
            &[&*update_authority],
            recent_blockhash,
        );

        client.send_and_confirm_transaction(&tx)?;
    }

    Ok(())
}

fn increment_key(key: &str, index: u32) -> String {
    (key.parse::<u32>()
        .expect("Key parsing out of bounds for u32.")
        + index)
        .to_string()
}
