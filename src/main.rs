use anchor_client::solana_sdk::pubkey::Pubkey;
use anyhow::{anyhow, Result};
use clap::Parser;
use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
    str::FromStr,
};
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

use mpl_candy_machine::CandyMachineData;

use sugar::cache::Cache;
use sugar::candy_machine::{get_candy_machine_state, print_candy_machine_state};
use sugar::cli::{Cli, Commands};
use sugar::mint::{process_mint_one, MintOneArgs};
use sugar::setup::sugar_setup;
use sugar::upload::{process_upload, UploadArgs};
use sugar::upload_assets::{process_upload_assets, UploadAssetsArgs};
use sugar::validate::{process_validate, ValidateArgs};
use sugar::verify::{process_verify, VerifyArgs};
use sugar::withdraw::{process_withdraw, WithdrawArgs};

pub fn default_candy_data() -> CandyMachineData {
    CandyMachineData {
        uuid: String::default(),
        price: u64::default(),
        symbol: String::default(),
        seller_fee_basis_points: u16::default(),
        max_supply: u64::default(),
        is_mutable: false,
        retain_authority: false,
        go_live_date: None,
        end_settings: None,
        creators: vec![],
        hidden_settings: None,
        whitelist_mint_settings: None,
        items_available: 1000,
        gatekeeper: None,
    }
}

fn setup_logging(level: Option<EnvFilter>) {
    // Log path; change this to be dynamic for multiple OSes.
    let log_path = PathBuf::from("/home/cilantro/.config/sugar/sugar.log");

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(&log_path)
        .unwrap();

    // Prioritize user-provided level, otherwise read from RUST_LOG env var for log level, fall back to "info" if not set.
    let env_filter = if let Some(filter) = level {
        filter
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    let formatting_layer = BunyanFormattingLayer::new("sugar".into(), file);

    let subscriber = Registry::default()
        .with(env_filter)
        .with(formatting_layer)
        .with(JsonStorageLayer);

    set_global_default(subscriber).expect("Failed to set global default subscriber");
}

#[tokio::main(worker_threads = 4)]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level_error = Err(anyhow!(
        "Invalid log level: {:?}.\n Valid levels are: trace, debug, info, warn, error.",
        cli.log_level
    ));

    if let Some(env_filter) = cli.log_level {
        match EnvFilter::try_new(env_filter) {
            Ok(env_filter) => setup_logging(Some(env_filter)),
            Err(_) => return log_level_error,
        }
    } else {
        setup_logging(None);
    }

    tracing::info!("Lend me some sugar, I am your neighbor.");

    match cli.command {
        Commands::MintOne { keypair, rpc_url } => {
            process_mint_one(MintOneArgs { keypair, rpc_url })?
        }
        Commands::Upload {
            assets_dir,
            arloader_manifest,
            config,
            keypair,
            rpc_url,
            cache,
        } => process_upload(UploadArgs {
            assets_dir,
            arloader_manifest,
            config,
            keypair,
            rpc_url,
            cache,
        })?,
        Commands::UploadAssets {
            assets_dir,
            config,
            keypair,
            rpc_url,
            cache,
        } => {
            process_upload_assets(UploadAssetsArgs {
                assets_dir,
                config,
                keypair,
                rpc_url,
                cache,
            })
            .await?
        }
        Commands::Test => process_test_command(),
        Commands::Validate { assets_dir, strict } => {
            process_validate(ValidateArgs { assets_dir, strict })?
        }
        Commands::Withdraw {
            candy_machine,
            keypair,
            rpc_url,
        } => process_withdraw(WithdrawArgs {
            candy_machine,
            keypair,
            rpc_url,
        })?,
        Commands::Verify {
            keypair,
            rpc_url,
            cache,
        } => process_verify(VerifyArgs {
            keypair,
            rpc_url,
            cache,
        })?,
    }

    Ok(())
}

fn process_test_command() {
    let sugar_config = sugar_setup(None, None).unwrap();
    let file = File::open("cache.json").unwrap();
    let cache: Cache = serde_json::from_reader(file).unwrap();

    let candy_machine_id = Pubkey::from_str(&cache.program.candy_machine).unwrap();
    let state = get_candy_machine_state(&sugar_config, &candy_machine_id).unwrap();

    print_candy_machine_state(state);
}
