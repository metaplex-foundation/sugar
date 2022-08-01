use std::fs::OpenOptions;

use console::style;
use sha2::{Digest, Sha256};

use crate::{
    cache::load_cache,
    common::*,
    config::{get_config_data, ConfigData, HiddenSettings},
};

pub struct HashArgs {
    pub config: String,
    pub cache: String,
    pub compare: Option<String>,
}

pub fn process_hash(args: HashArgs) -> Result<()> {
    let cache = load_cache(&args.cache, false)?;
    let mut config_data = get_config_data(&args.config)?;

    // We use std::process::exit to exit the program without going to the main handling which prints
    // "Command successful".

    if let Some(hash) = args.compare {
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_vec(&cache)?);
        let hash_base58 = bs58::encode(&hasher.finalize()).into_string();
        let expected_hash = hash_base58.chars().take(32).collect::<String>();
        if hash != expected_hash {
            println!(
                "{} {}",
                ERROR_EMOJI,
                style("Hashes do not match!").red().bold()
            );
            std::process::exit(0);
        }
        println!(
            "{} {}",
            COMPLETE_EMOJI,
            style("Hashes match!").blue().bold()
        );
        std::process::exit(0);
    }

    if let Some(ref hidden_settings) = config_data.hidden_settings {
        println!(
            "hash: {}",
            hash_and_update(
                hidden_settings.clone(),
                args.config,
                &mut config_data,
                &cache,
            )?
        );
        println!(
            "{} {}",
            COMPLETE_EMOJI,
            style("Config file updated with hash!").blue().bold()
        );
        std::process::exit(0);
    } else {
        return Err(anyhow!("No hidden settings found in config file."));
    }
}

pub fn hash_and_update(
    mut hidden_settings: HiddenSettings,
    config_file: String,
    config_data: &mut ConfigData,
    cache_data: &Cache,
) -> Result<String> {
    let mut hasher = Sha256::new();

    hasher.update(serde_json::to_vec(&cache_data)?);
    let hash_base58 = bs58::encode(&hasher.finalize()).into_string();

    let hash = hash_base58.chars().take(32).collect::<String>();
    // Candy machine only allows for 32 characters so we truncate this hash.
    hidden_settings.set_hash(hash.clone());
    config_data.hidden_settings = Some(hidden_settings);

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(Path::new(&config_file))?;

    serde_json::to_writer_pretty(file, &config_data)?;

    Ok(hash)
}