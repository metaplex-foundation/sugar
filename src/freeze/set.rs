use super::*;

pub struct SetFreezeArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub config: String,
    pub candy_machine: Option<String>,
}

pub fn process_set_freeze(args: SetFreezeArgs) -> Result<()> {
    let sugar_config = sugar_setup(args.keypair.clone(), args.rpc_url.clone())?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID);
    let config_data = get_config_data(&args.config)?;

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

    if config_data.freeze_time.is_none() {
        return Err(anyhow!("Freeze time is not set in config"));
    }

    println!(
        "{} {}Loading candy machine",
        style("[1/2]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );
    println!("{} {}", style("Candy machine ID:").bold(), candy_machine_id);

    let pb = spinner_with_style();
    pb.set_message("Connecting...");
    let candy_machine_state =
        get_candy_machine_state(&sugar_config, &Pubkey::from_str(&candy_machine_id)?)?;

    pb.finish_with_message("Done");

    assert_correct_authority(
        &sugar_config.keypair.pubkey(),
        &candy_machine_state.authority,
    )?;

    // Cannot set freeze if minting has started.
    if candy_machine_state.items_redeemed > 0 {
        return Err(anyhow!("Cannot set freeze after minting has started"));
    }

    println!(
        "\n{} {}Turning on freeze feature for candy machine",
        style("[2/2]").bold().dim(),
        ICE_CUBE_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Sending set collection transaction...");

    let signature = set_freeze(&program, &candy_pubkey, config_data.freeze_time.unwrap())?;

    pb.finish_with_message(format!(
        "{} {}",
        style("Set freeze signature:").bold(),
        signature
    ));

    Ok(())
}

fn set_freeze(program: &Program, candy_machine_id: &Pubkey, freeze_time: i64) -> Result<Signature> {
    let (freeze_pda, _) = find_freeze_pda(candy_machine_id);

    let builder = program
        .request()
        .accounts(nft_accounts::SetFreeze {
            candy_machine: *candy_machine_id,
            authority: program.payer(),
            freeze_pda,
            system_program: system_program::ID,
        })
        .args(nft_instruction::SetFreeze { freeze_time });

    let sig = builder.send()?;

    Ok(sig)
}
