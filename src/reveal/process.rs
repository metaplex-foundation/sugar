use crate::common::*;
use crate::config::get_config_data;
use crate::{cache::load_cache, candy_machine::*, common::*, pdas::get_collection_pda, utils::*};
use console::style;

pub struct RevealArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub config: String,
}

pub fn process_reveal(args: RevealArgs) -> Result<()> {
    println!(
        "{} {}Looking up candy machine",
        style("[1/1]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let config = get_config_data(&args.config)?;

    // If it's not a Hidden Settings mint, return an error.
    let hidden_settings = if let Some(hidden_settings) = config.hidden_settings {
        hidden_settings
    } else {
        return Err(anyhow!("Candy machine is not a Hidden Settings mint."));
    };

    let cache = load_cache(&args.cache, false)?;

    let sugar_config = sugar_setup(args.keypair, args.rpc_url)?;
    let client = setup_client(&sugar_config)?;
    let program = client.program(CANDY_MACHINE_ID);

    let candy_machine_id = match Pubkey::from_str(&cache.program.candy_machine) {
        Ok(candy_machine_id) => candy_machine_id,
        Err(_) => {
            let error = anyhow!(
                "Failed to parse candy machine id: {}",
                &cache.program.candy_machine
            );
            error!("{:?}", error);
            return Err(error);
        }
    };

    let cndy_state = get_candy_machine_state(&sugar_config, &candy_machine_id)?;

    pb.finish_and_clear();


    println!(
        "\n{}{} {}",
        CANDY_EMOJI,
        style("Candy machine ID:").dim(),
        &candy_machine_id
    );


    // Get all mint addresses for the candy machine.
    // Convert cache to use NFT names as the keys.
    // Look up each NFT by name get new URI for it.
    // Update NFT metadata with values from the config file and the new URI
    Ok(())
}
