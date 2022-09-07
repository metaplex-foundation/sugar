use anchor_client::solana_sdk::pubkey::Pubkey;
use anyhow::Result;
use console::style;
use mpl_candy_machine::{accounts as nft_accounts, instruction as nft_instruction};
use solana_program::instruction::AccountMeta;
use spl_associated_token_account::{create_associated_token_account, get_associated_token_address};

use crate::{
    cache::load_cache,
    candy_machine::{get_candy_machine_state, CANDY_MACHINE_ID},
    common::*,
    config::{get_config_data, ConfigData},
    pdas::*,
    utils::{assert_correct_authority, spinner_with_style},
};

pub mod remove;
pub mod set;
pub mod unlock;

pub use remove::*;
pub use set::*;
pub use unlock::*;
