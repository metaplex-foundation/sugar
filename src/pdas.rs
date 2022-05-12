use crate::candy_machine;
use anchor_client::solana_sdk::pubkey::Pubkey;
use mpl_token_metadata::pda::{find_master_edition_account, find_metadata_account};

pub fn get_metadata_pda(mint: &Pubkey) -> Pubkey {
    let (pda, _bump) = find_metadata_account(mint);

    pda
}

pub fn get_master_edition_pda(mint: &Pubkey) -> Pubkey {
    let (pda, _bump) = find_master_edition_account(mint);

    pda
}

pub fn get_candy_machine_creator_pda(candy_machine_id: &Pubkey) -> (Pubkey, u8) {
    // Derive metadata account
    let creator_seeds = &["candy_machine".as_bytes(), candy_machine_id.as_ref()];

    Pubkey::find_program_address(creator_seeds, &candy_machine::ID)
}

pub fn get_collection_pda(candy_machine_id: &Pubkey) -> (Pubkey, u8) {
    // Derive collection PDA address
    let collection_seeds = &["collection".as_bytes(), candy_machine_id.as_ref()];

    Pubkey::find_program_address(collection_seeds, &candy_machine::ID)
}
