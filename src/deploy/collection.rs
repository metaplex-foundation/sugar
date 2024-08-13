use anchor_client::solana_sdk::{compute_budget::ComputeBudgetInstruction, pubkey::Pubkey};
use anyhow::Result;
use mpl_core::instructions::{CreateCollectionV1, CreateCollectionV1InstructionArgs};

use crate::{
    candy_machine::CANDY_MACHINE_ID, common::*, config::ConfigData, deploy::DeployArgs,
    setup::SugarClient,
};

pub fn create_collection(
    client: &SugarClient,
    _candy_machine: Pubkey,
    cache: &mut Cache,
    _config_data: &ConfigData,
    args: &DeployArgs,
) -> Result<(Signature, Pubkey)> {
    let program = client.program(CANDY_MACHINE_ID)?;
    let payer = program.payer();

    let collection_mint = Keypair::new();
    let collection_item: &mut CacheItem = match cache.items.get_mut("-1") {
        Some(item) => item,
        None => {
            return Err(anyhow!("Trying to create and set collection when collection item info isn't in cache! This shouldn't happen!"));
        }
    };

    let create_metadata_account_ix = CreateCollectionV1 {
        collection: collection_mint.pubkey(),
        update_authority: None,
        payer,
        system_program: system_program::ID,
    }
    .instruction(CreateCollectionV1InstructionArgs {
        name: collection_item.name.clone(),
        uri: collection_item.metadata_link.clone(),
        plugins: None,
    });

    let priority_fee = ComputeBudgetInstruction::set_compute_unit_price(args.priority_fee);

    let builder = program
        .request()
        .instruction(priority_fee)
        .signer(&collection_mint)
        .instruction(create_metadata_account_ix);

    let sig = builder.send()?;

    collection_item.on_chain = true;
    cache.program.collection_mint = collection_mint.pubkey().to_string();
    cache.sync_file()?;

    Ok((sig, collection_mint.pubkey()))
}
