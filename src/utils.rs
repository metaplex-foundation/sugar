pub use anchor_client::solana_sdk::hash::Hash;
use anchor_client::{
    solana_sdk::{
        program_pack::{IsInitialized, Pack},
        pubkey::Pubkey,
    },
    Program,
};
pub use anyhow::{anyhow, Result};
pub use indicatif::{ProgressBar, ProgressStyle};
use solana_client::rpc_client::RpcClient;
use spl_token::state::{Account, Mint};
use std::str::FromStr;

use crate::config::data::Cluster;

/// Hash for devnet cluster
pub const DEVNET_HASH: &str = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";

/// Hash for mainnet-beta cluster
pub const MAINNET_HASH: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";

/// Return the environment of the current connected RPC.
pub fn get_cluster(rpc_client: RpcClient) -> Result<Cluster> {
    let devnet_hash = Hash::from_str(DEVNET_HASH).unwrap();
    let mainnet_hash = Hash::from_str(MAINNET_HASH).unwrap();
    let genesis_hash = rpc_client.get_genesis_hash()?;

    if genesis_hash == devnet_hash {
        Ok(Cluster::Devnet)
    } else if genesis_hash == mainnet_hash {
        Ok(Cluster::Mainnet)
    } else {
        Err(anyhow!(format!(
            "Genesis hash '{}' doesn't match supported Solana clusters for Bundlr",
            genesis_hash
        )))
    }
}

/// Return the environment of the current connected RPC.
pub fn check_spl_token(program: &Program, input: &String) -> Result<(), String> {
    let pubkey = Pubkey::from_str(&input).unwrap();
    let token_data = program.rpc().get_account_data(&pubkey).unwrap();
    let token_mint = Mint::unpack_from_slice(&token_data).unwrap();
    if !token_mint.is_initialized {
        let message = "The specified spl-token is not initialized.";
        Err(message.to_string())
    } else {
        Ok(())
    }
}

pub fn spinner_with_style() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(120);
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&[
                "▹▹▹▹▹",
                "▸▹▹▹▹",
                "▹▸▹▹▹",
                "▹▹▸▹▹",
                "▹▹▹▸▹",
                "▹▹▹▹▸",
                "▪▪▪▪▪",
            ])
            .template("{spinner:.dim} {msg}"),
    );
    pb
}

pub fn progress_bar_with_style(len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len);
    // forces the progress bar to show immediatly
    pb.tick();
    pb.enable_steady_tick(1000);
    pb.set_style(
        ProgressStyle::default_bar().template("[{elapsed_precise}] {msg}{wide_bar} {pos}/{len}"),
    );
    pb
}
