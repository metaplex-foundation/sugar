use std::sync::Arc;

use crate::common::*;
use crate::config::get_config_data;
use crate::parse::parse_solana_config;
use crate::pdas::find_metadata_pda;
use crate::{cache::load_cache, utils::*};
use anchor_client::solana_sdk::account::Account;
use anchor_lang::AnchorDeserialize;
use console::style;
use futures::future::join_all;
use mpl_token_metadata::{
    instruction::update_metadata_accounts_v2,
    state::{DataV2, Metadata},
    ID as TOKEN_METADATA_PROGRAM_ID,
};
use solana_client::client_error::ClientError;
use solana_client::rpc_client::RpcClient;
use solana_crawler::crawler::Crawler;
use tokio::sync::Semaphore;

pub struct RevealArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub config: String,
}

pub struct MetadataUpdateValues {
    pub metadata_pubkey: Pubkey,
    pub metadata: Metadata,
    pub new_uri: String,
}

pub async fn process_reveal(args: RevealArgs) -> Result<()> {
    println!(
        "{} {}Loading items from the cache",
        style("[1/4]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let config = get_config_data(&args.config)?;

    // If it's not a Hidden Settings mint, return an error.
    let _hidden_settings = if let Some(hidden_settings) = config.hidden_settings {
        hidden_settings
    } else {
        return Err(anyhow!("Candy machine is not a Hidden Settings mint."));
    };

    let cache = load_cache(&args.cache, false)?;
    let sugar_config = sugar_setup(args.keypair, args.rpc_url.clone())?;

    // Setup standard Solana client for the Crawler.
    let sol_config_option = parse_solana_config();

    let rpc_url = match args.rpc_url {
        Some(rpc_url) => rpc_url,
        None => match sol_config_option {
            Some(ref sol_config) => sol_config.json_rpc_url.clone(),
            None => String::from(DEFAULT_RPC_DEVNET),
        },
    };
    let client = RpcClient::new(&rpc_url);

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

    pb.finish_and_clear();

    println!(
        "{} {}Getting minted NFTs for candy machine {}",
        style("[2/4]").bold().dim(),
        LOOKING_GLASS_EMOJI,
        candy_machine_id
    );

    let spinner = spinner_with_style();
    spinner.set_message("Crawling candy machine id transactions...");
    let metadata_addresses = Crawler::get_cmv2_metadata(client, candy_machine_id).await?;
    spinner.finish_and_clear();

    println!(
        "{} {}Matching NFTs to cache values",
        style("[3/4]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );
    let metadata_pubkeys: Vec<Pubkey> = metadata_addresses
        .iter()
        .map(|a| Pubkey::from_str(a).unwrap())
        .collect();

    let mut futures = Vec::new();
    let client = Arc::new(RpcClient::new(&rpc_url));

    // Get all metadata accounts.
    metadata_pubkeys.as_slice().chunks(100).for_each(|chunk| {
        let client = client.clone();
        futures.push(async move { async_get_multiple_accounts(client, chunk).await });
    });
    let results = join_all(futures).await;
    let mut accounts = Vec::new();

    for result in results {
        let res = result.unwrap();
        accounts.extend(res);
    }

    let metadata: Vec<Metadata> = accounts
        .into_iter()
        .map(|a| a.unwrap().data)
        .map(|d| Metadata::deserialize(&mut d.as_slice()).unwrap())
        .collect();

    // Convert cache to use NFT names as the keys.
    let nft_lookup: HashMap<&String, &CacheItem> = cache
        .items
        .values()
        .map(|item| (&item.name, item))
        .collect();

    let mut update_values = Vec::new();

    println!(
        "{} {}Updating NFT URIs from cache values",
        style("[4/4]").bold().dim(),
        UPLOAD_EMOJI
    );

    let spinner = spinner_with_style();
    spinner.set_message("Setting up transactions...");
    for m in metadata {
        let name = m.data.name.trim_matches(char::from(0)).to_string();
        let metadata_pubkey = find_metadata_pda(&m.mint);
        let new_uri = nft_lookup.get(&name).unwrap().metadata_link.clone();
        update_values.push(MetadataUpdateValues {
            metadata_pubkey,
            metadata: m,
            new_uri,
        });
    }
    spinner.finish_and_clear();

    let spinner = spinner_with_style();
    spinner.set_message("Running network requests and awaiting results...");
    let keypair = Arc::new(sugar_config.keypair);
    let sem = Arc::new(Semaphore::new(1000));
    let mut tx_tasks = Vec::new();
    let mut transactions = Vec::new();

    for item in update_values {
        let permit = Arc::clone(&sem).acquire_owned().await.unwrap();
        let client = client.clone();
        let keypair = keypair.clone();
        tx_tasks.push(tokio::spawn(async move {
            // Move permit into the closure so it is dropped when the task is dropped.
            let _permit = permit;
            update_metadata_value(client, keypair, item).await
        }));
    }

    for task in tx_tasks {
        let res = task.await.unwrap();
        if let Ok(tx) = res {
            transactions.push(tx);
        }
    }
    spinner.finish_and_clear();

    println!("{}Reveal complete!", CONFETTI_EMOJI);

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
    value: MetadataUpdateValues,
) -> Result<(), ClientError> {
    let mut data = value.metadata.data;
    if data.uri.trim_matches(char::from(0)) != value.new_uri.trim_matches(char::from(0)) {
        data.uri = value.new_uri;

        let data_v2 = DataV2 {
            name: data.name,
            symbol: data.symbol,
            uri: data.uri,
            seller_fee_basis_points: data.seller_fee_basis_points,
            creators: data.creators,
            collection: value.metadata.collection,
            uses: value.metadata.uses,
        };

        let ix = update_metadata_accounts_v2(
            TOKEN_METADATA_PROGRAM_ID,
            value.metadata_pubkey,
            update_authority.pubkey(),
            None,
            Some(data_v2),
            None,
            None,
        );

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
