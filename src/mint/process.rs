use std::{str::FromStr, sync::Arc};

use anchor_client::solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    system_program, sysvar,
};
use anyhow::Result;
use console::style;
use mpl_candy_machine_core::{
    accounts as nft_accounts, instruction as nft_instruction, AccountVersion, CandyMachine,
};
use mpl_token_metadata::{
    instruction::MetadataDelegateRole,
    pda::{
        find_collection_authority_account, find_metadata_delegate_record_account,
        find_token_record_account,
    },
    state::{Metadata, TokenMetadataAccount},
};
use solana_client::rpc_response::Response;
use spl_associated_token_account::get_associated_token_address;
use spl_token::ID as TOKEN_PROGRAM_ID;
use tokio::sync::Semaphore;

use crate::{
    cache::load_cache,
    candy_machine::{CANDY_MACHINE_ID, *},
    common::*,
    config::{Cluster, SugarConfig},
    pdas::*,
    utils::*,
};

pub struct MintArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub number: Option<u64>,
    pub receiver: Option<String>,
    pub candy_machine: Option<String>,
    pub priority_fee: u64,
}

pub async fn process_mint(args: MintArgs) -> Result<()> {
    let sugar_config = sugar_setup(args.keypair, args.rpc_url)?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID);

    // the candy machine id specified takes precedence over the one from the cache

    let candy_machine_id = match args.candy_machine {
        Some(candy_machine_id) => candy_machine_id,
        None => {
            let cache = load_cache(&args.cache, false)?;
            cache.program.candy_machine
        }
    };

    let candy_pubkey = match Pubkey::from_str(&candy_machine_id) {
        Ok(candy_pubkey) => candy_pubkey,
        Err(_) => {
            let error = anyhow!("Failed to parse candy machine id: {}", candy_machine_id);
            error!("{:?}", error);
            return Err(error);
        }
    };

    println!(
        "{} {}Loading candy machine",
        style("[1/2]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );
    println!("{} {}", style("Candy machine ID:").bold(), candy_machine_id);

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let candy_machine_state = Arc::new(get_candy_machine_state(&sugar_config, &candy_pubkey)?);
    let (_, collection_metadata) =
        get_metadata_pda(&candy_machine_state.collection_mint, &program)?;
    let collection_update_authority = collection_metadata.update_authority;

    pb.finish_with_message("Done");

    println!(
        "\n{} {}Minting from candy machine",
        style("[2/2]").bold().dim(),
        CANDY_EMOJI
    );

    let receiver_pubkey = match args.receiver {
        Some(receiver_id) => Pubkey::from_str(&receiver_id)
            .map_err(|_| anyhow!("Failed to parse receiver pubkey: {}", receiver_id))?,
        None => sugar_config.keypair.pubkey(),
    };
    println!("\nMinting to {}", &receiver_pubkey);

    let number = args.number.unwrap_or(1);
    let available = candy_machine_state.data.items_available - candy_machine_state.items_redeemed;

    if number > available || number == 0 {
        let error = anyhow!("{} item(s) available, requested {}", available, number);
        error!("{:?}", error);
        return Err(error);
    }

    info!("Minting NFT from candy machine: {}", &candy_machine_id);
    info!("Candy machine program id: {:?}", CANDY_MACHINE_ID);

    if number == 1 {
        let pb = spinner_with_style();
        pb.set_message(format!(
            "{} item(s) remaining",
            candy_machine_state.data.items_available - candy_machine_state.items_redeemed
        ));
        let config = Arc::new(sugar_config);

        let result = match mint(
            Arc::clone(&config),
            candy_pubkey,
            Arc::clone(&candy_machine_state),
            collection_update_authority,
            receiver_pubkey,
            args.priority_fee,
        )
        .await
        {
            Ok((signature, mint)) => {
                println!("Mint: {mint}");
                println!("Signature: {signature}");
                format!("{}", style("Mint success").bold())
            }
            Err(err) => {
                pb.abandon_with_message(format!("{}", style("Mint failed ").red().bold()));
                error!("{:?}", err);
                return Err(err);
            }
        };

        pb.finish_with_message(result);
    } else {
        let pb = progress_bar_with_style(number);

        let mut tasks = Vec::new();
        let semaphore = Arc::new(Semaphore::new(10));
        let config = Arc::new(sugar_config);

        for _i in 0..number {
            let config = config.clone();
            let permit = Arc::clone(&semaphore).acquire_owned().await.unwrap();
            let candy_machine_state = candy_machine_state.clone();
            let pb = pb.clone();

            // Start tasks
            tasks.push(tokio::spawn(async move {
                let _permit = permit;
                let res = mint(
                    config,
                    candy_pubkey,
                    candy_machine_state,
                    collection_update_authority,
                    receiver_pubkey,
                    args.priority_fee,
                )
                .await;
                pb.inc(1);
                res
            }));
        }

        let mut error_count = 0;

        // Resolve tasks
        for task in tasks {
            let res = task.await.unwrap();
            if let Err(e) = res {
                error_count += 1;
                error!("{:?}, continuing. . .", e);
            }
        }

        if error_count > 0 {
            pb.abandon_with_message(format!(
                "{} {} items failed.",
                style("Some of the items failed to mint.").red().bold(),
                error_count
            ));
            return Err(anyhow!(
                "{} {}/{} {}",
                style("Minted").red().bold(),
                number - error_count,
                number,
                style("of the items").red().bold()
            ));
        }
        pb.finish();
    }

    Ok(())
}

pub async fn mint(
    config: Arc<SugarConfig>,
    candy_machine_id: Pubkey,
    candy_machine_state: Arc<CandyMachine>,
    collection_update_authority: Pubkey,
    receiver: Pubkey,
    priority_fee: u64,
) -> Result<(Signature, Pubkey)> {
    let client = setup_client(&config)?;
    let program = client.program(CANDY_MACHINE_ID);
    let payer = program.payer();

    if candy_machine_state.mint_authority != payer {
        return Err(anyhow!(
            "Payer is not the Candy Machine mint authority, mint disallowed."
        ));
    }

    let nft_mint = Keypair::new();
    let metaplex_program_id = Pubkey::from_str(METAPLEX_PROGRAM_ID)?;
    // derive associated token account
    let token = get_associated_token_address(&receiver, &nft_mint.pubkey());

    let collection_mint = candy_machine_state.collection_mint;

    let (authority_pda, _) = find_candy_machine_creator_pda(&candy_machine_id);

    let (token_record, collection_delegate_record) =
        if matches!(candy_machine_state.version, AccountVersion::V1) {
            (
                None,
                find_collection_authority_account(&collection_mint, &authority_pda).0,
            )
        } else {
            (
                Some(find_token_record_account(&nft_mint.pubkey(), &token).0),
                find_metadata_delegate_record_account(
                    &collection_mint,
                    MetadataDelegateRole::Collection,
                    &collection_update_authority,
                    &authority_pda,
                )
                .0,
            )
        };

    let collection_metadata = find_metadata_pda(&collection_mint);

    let data = program.rpc().get_account_data(&collection_metadata)?;
    let metadata = Metadata::safe_deserialize(data.as_slice())?;

    let metadata_pda = find_metadata_pda(&nft_mint.pubkey());
    let master_edition_pda = find_master_edition_pda(&nft_mint.pubkey());

    let compute_units = ComputeBudgetInstruction::set_compute_unit_limit(COMPUTE_UNITS);
    let priority_fee_ix = ComputeBudgetInstruction::set_compute_unit_price(priority_fee);

    let mint_ix = program
        .request()
        .instruction(compute_units)
        .instruction(priority_fee_ix)
        .accounts(nft_accounts::MintV2 {
            candy_machine: candy_machine_id,
            authority_pda,
            payer,
            nft_owner: receiver,
            token: Some(token),
            token_record,
            mint_authority: payer,
            nft_metadata: metadata_pda,
            nft_mint: nft_mint.pubkey(),
            nft_master_edition: master_edition_pda,
            nft_mint_authority: payer,
            collection_mint: candy_machine_state.collection_mint,
            collection_metadata: find_metadata_pda(&candy_machine_state.collection_mint),
            collection_master_edition: find_master_edition_pda(
                &candy_machine_state.collection_mint,
            ),
            collection_delegate_record,
            collection_update_authority: metadata.update_authority,
            token_metadata_program: metaplex_program_id,
            spl_token_program: TOKEN_PROGRAM_ID,
            spl_ata_program: Some(spl_associated_token_account::ID),
            system_program: system_program::id(),
            sysvar_instructions: sysvar::instructions::ID,
            recent_slothashes: sysvar::slot_hashes::ID,
            authorization_rules_program: None,
            authorization_rules: None,
        })
        .args(nft_instruction::MintV2 {});

    let mut mint_ix = mint_ix.instructions()?;

    for account_meta in &mut mint_ix[0].accounts {
        if account_meta.pubkey == nft_mint.pubkey() {
            account_meta.is_signer = true;
            account_meta.is_writable = true;
        }
    }

    // need to increase the number of compute units
    let compute_units = ComputeBudgetInstruction::set_compute_unit_limit(COMPUTE_UNITS);
    let priority_fee_ix = ComputeBudgetInstruction::set_compute_unit_price(priority_fee);

    let builder = program
        .request()
        .instruction(compute_units)
        .instruction(priority_fee_ix)
        .instruction(mint_ix[0].clone())
        .signer(&nft_mint);

    let sig = builder.send()?;

    if let Err(_) | Ok(Response { value: None, .. }) = program
        .rpc()
        .get_account_with_commitment(&metadata_pda, CommitmentConfig::processed())
    {
        let cluster_param = match get_cluster(program.rpc()).unwrap_or(Cluster::Mainnet) {
            Cluster::Devnet => "?devnet",
            _ => "",
        };
        return Err(anyhow!(
            "Minting most likely failed with a bot tax. Check the transaction link for more details: https://explorer.solana.com/tx/{}{}",
            sig.to_string(),
            cluster_param,
        ));
    }

    info!("Minted! TxId: {}", sig);

    Ok((sig, nft_mint.pubkey()))
}
