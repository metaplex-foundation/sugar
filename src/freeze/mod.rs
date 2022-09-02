use anchor_client::solana_sdk::pubkey::Pubkey;
use anyhow::Result;
use console::style;
use mpl_candy_machine::{accounts as nft_accounts, instruction as nft_instruction};

use crate::{
    cache::load_cache,
    candy_machine::{get_candy_machine_state, CANDY_MACHINE_ID},
    common::*,
    config::get_config_data,
    pdas::*,
    utils::{assert_correct_authority, spinner_with_style},
};

pub mod set;

pub use set::*;
