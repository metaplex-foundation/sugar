use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use anchor_client::solana_sdk::pubkey::Pubkey;
use anyhow::Result;
use console::style;
use mpl_core_candy_guard::{
    guards::FreezeEscrow,
    state::{CandyGuardData, DATA_OFFSET},
};
use serde::{Deserialize, Serialize, Serializer};
use solana_client::rpc_client::RpcClient;
use solana_program::{instruction::AccountMeta, program_pack::Pack};
use spl_associated_token_account::get_associated_token_address;
use tokio::sync::Semaphore;

use crate::{
    cache::load_cache,
    common::*,
    config::{get_config_data, Cluster, ConfigData, SugarConfig},
    pdas::*,
    setup::get_rpc_url,
    utils::{get_cluster, progress_bar_with_style, spinner_with_style},
};

mod initialize;
mod thaw;
mod unlock_funds;

pub use initialize::*;
pub use thaw::*;
pub use unlock_funds::*;

pub fn find_freeze_pda(
    candy_guard_id: &Pubkey,
    candy_machine_id: &Pubkey,
    destination: &Pubkey,
) -> (Pubkey, u8) {
    let freeze_seeds = &[
        FreezeEscrow::PREFIX_SEED,
        destination.as_ref(),
        candy_guard_id.as_ref(),
        candy_machine_id.as_ref(),
    ];

    Pubkey::find_program_address(freeze_seeds, &mpl_core_candy_guard::ID)
}

pub fn get_destination<C: Deref<Target = impl Signer> + Clone>(
    program: &Program<C>,
    candy_guard: &Pubkey,
    config_data: ConfigData,
    label: &Option<String>,
) -> Result<(Pubkey, Option<Pubkey>)> {
    // first tries to get the on-chain information

    if let Ok(account_data) = program.rpc().get_account_data(candy_guard) {
        // deserialises the candy guard data
        let candy_guard_data = CandyGuardData::load(&account_data[DATA_OFFSET..])?;

        match &label {
            // if we have a label, need to find the group
            Some(label) => {
                let clone = label.to_owned();
                if let Some(groups) = &candy_guard_data.groups {
                    for group in groups {
                        if group.label == clone {
                            if let Some(guard) = &group.guards.freeze_sol_payment {
                                return Ok((guard.destination, None));
                            }
                            if let Some(guard) = candy_guard_data.default.freeze_token_payment {
                                return Ok((guard.destination_ata, Some(guard.mint)));
                            }
                        }
                    }
                }
            }
            None => {
                if let Some(guard) = candy_guard_data.default.freeze_sol_payment {
                    return Ok((guard.destination, None));
                }
                if let Some(guard) = candy_guard_data.default.freeze_token_payment {
                    return Ok((guard.destination_ata, Some(guard.mint)));
                }
            }
        }
    }

    // if on-chain was not successful, check the config data

    if let Some(guards) = config_data.guards {
        match &label {
            // if we have a label, need to find the group
            Some(label) => {
                let clone = label.to_owned();
                if let Some(groups) = &guards.groups {
                    for group in groups {
                        if group.label == clone {
                            return if let Some(guard) = &group.guards.freeze_sol_payment {
                                Ok((guard.destination, None))
                            } else if let Some(guard) = &group.guards.freeze_token_payment {
                                Ok((guard.destination_ata, Some(guard.mint)))
                            } else {
                                Err(anyhow!(
                                    "Missing freeze payment guard for group with label '{label}'"
                                ))
                            };
                        }
                    }
                    // reaching this point means that we did not find the group
                    Err(anyhow!("Could not find group with label '{label}'"))
                } else {
                    Err(anyhow!("Missig group configuration"))
                }
            }
            None => {
                if let Some(guard) = guards.default.freeze_sol_payment {
                    Ok((guard.destination, None))
                } else if let Some(guard) = guards.default.freeze_token_payment {
                    Ok((guard.destination_ata, Some(guard.mint)))
                } else {
                    Err(anyhow!("Missing freeze payment guard configuration"))
                }
            }
        }
    } else {
        Err(anyhow!("Missing guards configuration"))
    }
}
