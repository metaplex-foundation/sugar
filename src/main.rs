use std::{
    fs::OpenOptions,
    path::PathBuf,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, Result};
use clap::Parser;
use console::style;
use sugar_cli::{
    airdrop::{process_airdrop, AirdropArgs},
    bundlr::{process_bundlr, BundlrArgs},
    cli::{Cli, CollectionSubcommands, Commands, ConfigSubcommands, FreezeCommand, GuardCommand},
    collections::{process_set_collection, SetCollectionArgs},
    constants::{COMPLETE_EMOJI, ERROR_EMOJI},
    create_config::{process_create_config, CreateConfigArgs},
    deploy::{process_deploy, DeployArgs},
    freeze::{
        process_initialize, process_thaw, process_unlock_funds, InitializeArgs, ThawArgs,
        UnlockFundsArgs,
    },
    guard::{
        process_guard_add, process_guard_remove, process_guard_show, process_guard_update,
        process_guard_withdraw, GuardAddArgs, GuardRemoveArgs, GuardShowArgs, GuardUpdateArgs,
        GuardWithdrawArgs,
    },
    hash::{process_hash, HashArgs},
    launch::{process_launch, LaunchArgs},
    mint::{process_mint, MintArgs},
    parse::parse_sugar_errors,
    reveal::{process_reveal, RevealArgs},
    show::{process_show, ShowArgs},
    sign::{process_sign, SignArgs},
    update::{process_set_token_stardard, process_update, SetTokenStandardArgs, UpdateArgs},
    upload::{process_upload, UploadArgs},
    validate::{process_validate, ValidateArgs},
    verify::{process_verify, VerifyArgs},
    withdraw::{process_withdraw, WithdrawArgs},
};
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{self, filter::LevelFilter, prelude::*, EnvFilter};

fn setup_logging(level: Option<EnvFilter>) -> Result<()> {
    // Log path; change this to be dynamic for multiple OSes.
    // Log in current directory for now.
    let log_path = PathBuf::from("sugar.log");

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(log_path)
        .unwrap();

    // Prioritize user-provided level, otherwise read from RUST_LOG env var for log level, fall back to "tracing" if not set.
    let env_filter = if let Some(filter) = level {
        filter
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"))
    };

    let formatting_layer = BunyanFormattingLayer::new("sugar".into(), file);
    let level_filter = LevelFilter::from_str(&env_filter.to_string())?;

    let subscriber = tracing_subscriber::registry()
        .with(formatting_layer.with_filter(level_filter))
        .with(JsonStorageLayer);

    set_global_default(subscriber).expect("Failed to set global default subscriber");

    Ok(())
}

#[tokio::main]
async fn main() {
    match run().await {
        Ok(()) => {
            println!(
                "\n{}{}",
                COMPLETE_EMOJI,
                style("Command successful.").green().bold().dim()
            );
        }
        Err(err) => {
            let parsed_err = parse_sugar_errors(&err.to_string());

            println!(
                "\n{}{} {}",
                ERROR_EMOJI,
                style("Error running command (re-run needed):").red(),
                parsed_err,
            );
            // finished the program with an error code to the OS
            std::process::exit(1);
        }
    }
}

async fn run() -> Result<()> {
    solana_logger::setup_with_default("solana=off");

    let cli = Cli::parse();

    let log_level_error: Result<()> = Err(anyhow!(
        "Invalid log level: {:?}.\n Valid levels are: trace, debug, info, warn, error.",
        cli.log_level
    ));

    if let Some(user_filter) = cli.log_level {
        let filter = match EnvFilter::from_str(&user_filter) {
            Ok(filter) => filter,
            Err(_) => return log_level_error,
        };
        setup_logging(Some(filter))?;
    } else {
        setup_logging(None)?;
    }

    tracing::info!("Lend me some sugar, I am your neighbor.");

    let interrupted = Arc::new(AtomicBool::new(true));
    let ctrl_handler = interrupted.clone();

    ctrlc::set_handler(move || {
        if ctrl_handler.load(Ordering::SeqCst) {
            // we really need to exit
            println!(
                "\n\n{}{} Operation aborted.",
                ERROR_EMOJI,
                style("Error running command (re-run needed):").red(),
            );
            // finished the program with an error code to the OS
            std::process::exit(1);
        }
        // signal that we want to exit
        ctrl_handler.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    match cli.command {
        Commands::Bundlr {
            keypair,
            rpc_url,
            action,
        } => {
            process_bundlr(BundlrArgs {
                keypair,
                rpc_url,
                action,
            })
            .await?
        }
        Commands::Collection { command } => match command {
            CollectionSubcommands::Set {
                keypair,
                rpc_url,
                cache,
                config,
                candy_machine,
                collection_mint,
                priority_fee,
            } => process_set_collection(SetCollectionArgs {
                collection_mint,
                keypair,
                rpc_url,
                cache,
                config,
                candy_machine,
                priority_fee,
            })?,
        },
        Commands::Config { command } => match command {
            ConfigSubcommands::Create {
                config,
                keypair,
                rpc_url,
                assets_dir,
            } => process_create_config(CreateConfigArgs {
                config,
                keypair,
                rpc_url,
                assets_dir,
            })?,
            ConfigSubcommands::Update {
                config,
                keypair,
                rpc_url,
                cache,
                new_authority,
                candy_machine,
                priority_fee,
            } => process_update(UpdateArgs {
                config,
                keypair,
                rpc_url,
                cache,
                new_authority,
                candy_machine,
                priority_fee,
            })?,
            ConfigSubcommands::Set {
                keypair,
                rpc_url,
                cache,
                token_standard,
                candy_machine,
                rule_set,
                priority_fee,
            } => process_set_token_stardard(SetTokenStandardArgs {
                keypair,
                rpc_url,
                cache,
                token_standard,
                candy_machine,
                rule_set,
                priority_fee,
            })?,
        },
        Commands::Deploy {
            config,
            keypair,
            rpc_url,
            cache,
            collection_mint,
            priority_fee,
        } => {
            process_deploy(DeployArgs {
                config,
                keypair,
                rpc_url,
                cache,
                interrupted: interrupted.clone(),
                collection_mint,
                priority_fee,
            })
            .await?
        }
        Commands::Freeze { command } => match command {
            FreezeCommand::Initialize {
                keypair,
                rpc_url,
                cache,
                config,
                candy_guard,
                candy_machine,
                label,
                period,
                priority_fee,
            } => process_initialize(InitializeArgs {
                keypair,
                rpc_url,
                cache,
                config,
                candy_guard,
                candy_machine,
                label,
                period,
                priority_fee,
            })?,
            FreezeCommand::Thaw {
                keypair,
                rpc_url,
                cache,
                config,
                all,
                nft_mint,
                candy_guard,
                candy_machine,
                destination,
                label,
                use_cache,
                timeout,
                token,
                priority_fee,
            } => {
                process_thaw(ThawArgs {
                    keypair,
                    rpc_url,
                    cache,
                    config,
                    all,
                    nft_mint,
                    candy_guard,
                    candy_machine,
                    destination,
                    label,
                    use_cache,
                    timeout,
                    token,
                    priority_fee,
                })
                .await?
            }
            FreezeCommand::UnlockFunds {
                keypair,
                rpc_url,
                cache,
                config,
                candy_guard,
                candy_machine,
                destination,
                label,
                token,
                priority_fee,
            } => process_unlock_funds(UnlockFundsArgs {
                keypair,
                rpc_url,
                cache,
                config,
                candy_guard,
                candy_machine,
                destination,
                label,
                token,
                priority_fee,
            })?,
        },
        Commands::Guard { command } => match command {
            GuardCommand::Add {
                keypair,
                rpc_url,
                cache,
                config,
                candy_machine,
                candy_guard,
                priority_fee,
            } => process_guard_add(GuardAddArgs {
                keypair,
                rpc_url,
                cache,
                config,
                candy_machine,
                candy_guard,
                priority_fee,
            })?,
            GuardCommand::Remove {
                keypair,
                rpc_url,
                cache,
                candy_machine,
                candy_guard,
                priority_fee,
            } => process_guard_remove(GuardRemoveArgs {
                keypair,
                rpc_url,
                cache,
                candy_machine,
                candy_guard,
                priority_fee,
            })?,
            GuardCommand::Show {
                keypair,
                rpc_url,
                cache,
                candy_guard,
            } => process_guard_show(GuardShowArgs {
                keypair,
                rpc_url,
                cache,
                candy_guard,
            })?,
            GuardCommand::Update {
                keypair,
                rpc_url,
                cache,
                config,
                candy_guard,
                priority_fee,
            } => process_guard_update(GuardUpdateArgs {
                keypair,
                rpc_url,
                cache,
                config,
                candy_guard,
                priority_fee,
            })?,
            GuardCommand::Withdraw {
                keypair,
                rpc_url,
                cache,
                candy_guard,
                priority_fee,
            } => process_guard_withdraw(GuardWithdrawArgs {
                keypair,
                rpc_url,
                cache,
                candy_guard,
                priority_fee,
            })?,
        },
        Commands::Hash {
            config,
            cache,
            compare,
        } => process_hash(HashArgs {
            config,
            cache,
            compare,
        })?,
        Commands::Launch {
            assets_dir,
            config,
            keypair,
            rpc_url,
            cache,
            strict,
            skip_collection_prompt,
            priority_fee,
        } => {
            process_launch(LaunchArgs {
                assets_dir,
                config,
                keypair,
                rpc_url,
                cache,
                strict,
                skip_collection_prompt,
                interrupted: interrupted.clone(),
                priority_fee,
            })
            .await?
        }
        Commands::Mint {
            keypair,
            rpc_url,
            cache,
            number,
            receiver,
            candy_machine,
            priority_fee,
        } => {
            process_mint(MintArgs {
                keypair,
                rpc_url,
                cache,
                number,
                receiver,
                candy_machine,
                priority_fee,
            })
            .await?
        }
        Commands::Airdrop {
            keypair,
            rpc_url,
            cache,
            candy_machine,
            airdrop_list,
            priority_fee,
        } => {
            process_airdrop(AirdropArgs {
                keypair,
                rpc_url,
                cache,
                candy_machine,
                airdrop_list,
                priority_fee,
            })
            .await?
        }
        Commands::Reveal {
            keypair,
            rpc_url,
            cache,
            config,
            timeout,
        } => {
            process_reveal(RevealArgs {
                keypair,
                rpc_url,
                cache,
                config,
                timeout,
            })
            .await?
        }
        Commands::Show {
            keypair,
            rpc_url,
            cache,
            candy_machine,
            unminted,
        } => process_show(ShowArgs {
            keypair,
            rpc_url,
            cache,
            candy_machine,
            unminted,
        })?,
        Commands::Upload {
            assets_dir,
            config,
            keypair,
            rpc_url,
            cache,
            priority_fee,
        } => {
            process_upload(UploadArgs {
                assets_dir,
                config,
                keypair,
                rpc_url,
                cache,
                interrupted: interrupted.clone(),
                priority_fee,
            })
            .await?
        }
        Commands::Validate {
            assets_dir,
            strict,
            skip_collection_prompt,
        } => process_validate(ValidateArgs {
            assets_dir,
            strict,
            skip_collection_prompt,
        })?,
        Commands::Verify {
            keypair,
            rpc_url,
            cache,
        } => process_verify(VerifyArgs {
            keypair,
            rpc_url,
            cache,
        })?,
        Commands::Withdraw {
            candy_machine,
            keypair,
            rpc_url,
            list,
            authority,
            priority_fee,
        } => process_withdraw(WithdrawArgs {
            candy_machine,
            keypair,
            rpc_url,
            list,
            authority,
            priority_fee,
        })?,
        Commands::Sign {
            keypair,
            rpc_url,
            cache,
            mint,
            candy_machine_id,
        } => {
            process_sign(SignArgs {
                keypair,
                rpc_url,
                cache,
                mint,
                candy_machine_id,
            })
            .await?
        }
    }

    Ok(())
}
