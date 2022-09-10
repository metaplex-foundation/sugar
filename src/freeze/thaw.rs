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

pub struct ThawAllArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub config: String,
    pub candy_machine: Option<String>,
}

#[allow(dead_code)]
struct ThawNft {
    mint: Pubkey,
    owner: Pubkey,
    token_account: Pubkey,
}

#[derive(Debug, Deserialize)]
pub struct JRpcResponse {
    value: Vec<TokenAccount>,
}

#[derive(Debug, Deserialize)]
struct TokenAccount {
    address: String,
    amount: String,
    // decimals: u8,
    // #[serde(rename = "uiAmount")]
    // ui_amount: f32,
    // #[serde(rename = "uiAmountString")]
    // ui_amount_string: String,
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

pub async fn process_thaw_all(args: ThawAllArgs) -> Result<()> {
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
        "\n{} {}Getting minted NFTs for candy machine {}",
        style("[2/4]").bold().dim(),
        LOOKING_GLASS_EMOJI,
        candy_machine_id
    );

    let solana_cluster: Cluster = get_cluster(program.rpc())?;
    let rpc_url = get_rpc_url(args.rpc_url);
    let client = RpcClient::new(&rpc_url);

    let mint_pubkeys = match solana_cluster {
        Cluster::Devnet => {
            let (creator, _) = find_candy_machine_creator_pda(&candy_pubkey);
            let creator = bs58::encode(creator).into_string();
            get_cm_creator_mint_accounts(&client, &creator, 0)?
        }
        Cluster::Mainnet => {
            // New client instance because we have to move it into the crawler.
            let client = RpcClient::new(&rpc_url);

            let crawled_accounts = Crawler::get_cmv2_mints(client, candy_pubkey).await?;
            match crawled_accounts.get("mint") {
                Some(accounts) => accounts
                    .iter()
                    .map(|account| Pubkey::from_str(account).unwrap())
                    .collect::<Vec<Pubkey>>(),
                None => Vec::new(),
            }
        }
        _ => {
            return Err(anyhow!(
                "Cluster being used is unsupported for this command."
            ))
        }
    };

    if mint_pubkeys.is_empty() {
        pb.finish_with_message(format!("{}", style("No NFTs found.").green().bold()));
        return Err(anyhow!(format!(
            "No NFTs found for candy machine id {candy_machine_id}.",
        )));
    } else {
        pb.finish_with_message(format!("Found {:?} accounts", mint_pubkeys.len() as u64));
    }

    let pb = progress_bar_with_style(mint_pubkeys.len() as u64);

    let semaphore = Arc::new(Semaphore::new(100));
    let client = Arc::new(client);

    let mut join_handles = Vec::new();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let thaw_nfts = Arc::new(Mutex::new(Vec::new()));

    for mint in mint_pubkeys {
        let permit = Arc::clone(&semaphore).acquire_owned().await.unwrap();
        let client = client.clone();
        let pb = pb.clone();
        let errors = errors.clone();
        let thaw_nfts = thaw_nfts.clone();

        join_handles.push(tokio::spawn(async move {
            let _permit = permit;

            let request = RpcRequest::Custom {
                method: "getTokenLargestAccounts",
            };
            let params = json!([mint.to_string(), { "commitment": "confirmed" }]);
            let result: JRpcResponse = client.send(request, params).unwrap();

            let token_accounts: Vec<TokenAccount> = result
                .value
                .into_iter()
                .filter(|account| account.amount.parse::<u64>().unwrap() == 1)
                .collect();

            if token_accounts.len() != 1 {
                errors.lock().unwrap().push(anyhow!(
                    "Mint account {} had more than one token account with 1 token",
                    mint
                ));
                return;
            }

            let token_account = Pubkey::from_str(&token_accounts[0].address).unwrap();
            let account = client
                .get_account_with_commitment(&token_account, CommitmentConfig::confirmed())
                .unwrap()
                .value
                .unwrap();
            let owner = account.owner;

            thaw_nfts.lock().unwrap().push(ThawNft {
                mint,
                token_account,
                owner,
            });

            pb.inc(1);
        }));
    }

    for handle in join_handles {
        handle
            .await
            .map_err(|err| errors.lock().unwrap().push(anyhow!(err)))
            .ok();
    }

    if !errors.lock().unwrap().is_empty() {
        pb.abandon_with_message(format!(
            "{}",
            style("Failed get all the necessary information to Thaw all NFTs.")
                .red()
                .bold()
        ));
        return Err(anyhow!("Not all NFTs were thawed.".to_string()));
    } else {
        pb.finish_with_message(format!(
            "{}",
            style("All NFTs thawed successfully.").green().bold()
        ));
    }

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
