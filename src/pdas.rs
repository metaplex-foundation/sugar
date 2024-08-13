use anchor_client::solana_sdk::pubkey::Pubkey;

use crate::candy_machine::CANDY_MACHINE_ID;

pub type PdaInfo<T> = (Pubkey, T);

pub struct CollectionPDA {
    pub mint: Pubkey,
    pub candy_machine: Pubkey,
}

pub fn find_candy_machine_creator_pda(candy_machine_id: &Pubkey) -> (Pubkey, u8) {
    // Derive metadata account
    let creator_seeds = &["candy_machine".as_bytes(), candy_machine_id.as_ref()];

    Pubkey::find_program_address(creator_seeds, &CANDY_MACHINE_ID)
}

pub fn find_collection_pda(candy_machine_id: &Pubkey) -> (Pubkey, u8) {
    // Derive collection PDA address
    let collection_seeds = &["collection".as_bytes(), candy_machine_id.as_ref()];

    Pubkey::find_program_address(collection_seeds, &CANDY_MACHINE_ID)
}
