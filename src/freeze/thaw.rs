use super::*;

pub struct ThawArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub config: String,
    pub candy_machine: Option<String>,
    pub nft_mint: String,
    pub owner: Option<String>,
}

pub fn process_thaw(args: ThawArgs) -> Result<()> {
    let sugar_config = sugar_setup(args.keypair.clone(), args.rpc_url.clone())?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID);

    // The candy machine id specified takes precedence over the one from the cache.
    let candy_machine_id = match args.candy_machine {
        Some(ref candy_machine_id) => candy_machine_id.to_owned(),
        None => {
            let cache = load_cache(&args.cache, false)?;
            cache.program.candy_machine
        }
    };

    let owner = match args.owner {
        Some(ref owner) => {
            Pubkey::from_str(owner).map_err(|_| anyhow!("Failed to parse owner as a pubkey"))?
        }
        None => program.payer(),
    };

    let candy_pubkey = Pubkey::from_str(&candy_machine_id)
        .map_err(|_| anyhow!("Failed to parse candy machine id: {}", &candy_machine_id))?;

    println!(
        "{} {}Loading candy machine",
        style("[1/2]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );
    println!("{} {}", style("Candy machine ID:").bold(), candy_machine_id);

    let pb = spinner_with_style();
    pb.set_message("Connecting...");
    let _candy_machine_state =
        get_candy_machine_state(&sugar_config, &Pubkey::from_str(&candy_machine_id)?)?;

    pb.finish_with_message("Done");

    println!(
        "\n{} {}Thawing NFT. . .",
        style("[2/2]").bold().dim(),
        MONEY_BAG_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Sending thaw transaction...");

    let nft_mint_pubkey = Pubkey::from_str(&args.nft_mint)
        .map_err(|_| anyhow!("Failed to parse nft mint id: {}", &args.nft_mint))?;

    let signature = thaw_nft(&program, &candy_pubkey, nft_mint_pubkey, owner)?;

    pb.finish_with_message(format!(
        "{} {}",
        style("Thaw NFT signature:").bold(),
        signature
    ));

    Ok(())
}

pub fn thaw_nft(
    program: &Program,
    candy_machine_id: &Pubkey,
    nft_mint: Pubkey,
    owner: Pubkey,
) -> Result<Signature> {
    let (freeze_pda, _) = find_freeze_pda(candy_machine_id);
    let edition = find_master_edition_pda(&nft_mint);
    let token_account = get_associated_token_address(&owner, &nft_mint);

    println!("Freeze PDA: {}", freeze_pda);

    let builder = program
        .request()
        .accounts(nft_accounts::ThawNFT {
            freeze_pda,
            candy_machine: *candy_machine_id,
            token_account,
            owner,
            mint: nft_mint,
            edition,
            payer: program.payer(),
            token_program: spl_token::ID,
            token_metadata_program: mpl_token_metadata::ID,
            system_program: system_program::ID,
        })
        .args(nft_instruction::ThawNft);

    let sig = builder.send()?;

    Ok(sig)
}
