use std::{
    collections::HashSet,
    fs::File,
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use console::{style, Style};
use dialoguer::{theme::ColorfulTheme, Confirm};
use glob::glob;
use rayon::prelude::*;
use regex::Regex;

use crate::{common::*, utils::*, validate::*};

pub struct ValidateArgs {
    pub assets_dir: String,
    pub strict: bool,
    pub skip_collection_prompt: bool,
}

pub fn process_validate(args: ValidateArgs) -> Result<()> {
    // loading assets
    println!(
        "{} {}Loading assets",
        style("[1/1]").bold().dim(),
        ASSETS_EMOJI
    );

    let assets_dir = Path::new(&args.assets_dir);

    // missing or empty assets directory
    if !assets_dir.exists() || assets_dir.read_dir()?.next().is_none() {
        info!("Assets directory is missing or empty.");
        return Err(ValidateParserError::MissingOrEmptyAssetsDirectory.into());
    }

    if !args.skip_collection_prompt {
        let collection_path = assets_dir.join("collection.json");
        if !collection_path.is_file() {
            let warning = format!(
                "+----------------------------------------------+\n\
                 | {} MISSING COLLECTION FILES IN ASSETS FOLDER |\n\
                 +----------------------------------------------+",
                WARNING_EMOJI
            );
            println!(
                "\n{}\n{}\n",
                style(warning).bold().yellow(),
                style(
                    "Check https://docs.metaplex.com/developer-tools/sugar/guides/preparing-assets for the collection file requirements \
                    if you want a collection to be set automatically."
                )
                .italic()
                .yellow()
            );

            let theme = ColorfulTheme {
                success_prefix: style("✔".to_string()).yellow().force_styling(true),
                values_style: Style::new().yellow(),
                ..get_dialoguer_theme()
            };

            if !Confirm::with_theme(&theme).with_prompt("Do you want to continue without automatically setting the candy machine collection?").interact()? {
                return Err(anyhow!("Operation aborted"));
            }
            println!();
        }
    }

    let errors = Arc::new(Mutex::new(Vec::new()));

    let path = assets_dir.join("*.json");
    let pattern = path
        .to_str()
        .ok_or(ValidateParserError::InvalidAssetsDirectory)?;

    // Unwrapping here because we know the pattern is valid and GlobErrors should
    // be rare or impossible to produce.
    let paths: Vec<PathBuf> = glob(pattern)
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
        .collect();

    // Checking the assets are a proper series starting at 0 and ending at n-1
    let num_re = Regex::new(r".*/(\d+).json$").unwrap();

    let num_series = paths
        .iter()
        .filter_map(|path| {
            let name = path.file_name().unwrap().to_str().unwrap();
            num_re
                .captures(name)
                .map(|number| number[1].parse::<usize>().unwrap())
        })
        .collect::<Vec<usize>>();

    // Sum of series given we expect:
    // a_0 = 0 , a_n = num_series.size() - 1 , n = num_series.size() => n * (a_0 + a_n) / 2
    // https://en.wikipedia.org/wiki/Arithmetic_progression

    let target_sum = num_series.len() * (num_series.len() - 1) / 2;
    let mut sum: usize = 0;
    let mut redundant: HashSet<usize> = HashSet::new();
    for num in &num_series {
        if redundant.contains(num) {
            return Err(ValidateParserError::RedundantFile(*num).into());
        } else if num >= &num_series.len() {
            return Err(ValidateParserError::FileOutOfRange(*num).into());
        } else {
            redundant.insert(*num);
            sum += num;
        }
    }

    if sum != target_sum {
        return Err(ValidateParserError::NonContinuousSeries.into());
    }

    let pb = spinner_with_style();
    pb.enable_steady_tick(120);
    pb.set_message(format!("Validating {} metadata file(s)...", paths.len()));

    paths.par_iter().for_each(|path| {
        let errors = errors.clone();
        let f = match File::open(path) {
            Ok(f) => f,
            Err(error) => {
                error!("{}: {}", path.display(), error);
                errors.lock().unwrap().push(ValidateError {
                    path,
                    error: error.to_string(),
                });
                return;
            }
        };

        let metadata = match serde_json::from_reader::<File, Metadata>(f) {
            Ok(metadata) => metadata,
            Err(error) => {
                error!("{}: {}", path.display(), error);
                errors.lock().unwrap().push(ValidateError {
                    path,
                    error: error.to_string(),
                });
                return;
            }
        };

        // To be replaced with the strict validator once JSON standard is finalized.
        if args.strict {
            match metadata.validate() {
                Ok(()) => {}
                Err(e) => {
                    error!("{}: {}", path.display(), e);
                    errors.lock().unwrap().push(ValidateError {
                        path,
                        error: e.to_string(),
                    });
                }
            }
        } else {
            match metadata.validate() {
                Ok(()) => {}
                Err(e) => {
                    error!("{}: {}", path.display(), e);
                    errors.lock().unwrap().push(ValidateError {
                        path,
                        error: e.to_string(),
                    });
                }
            }
        }
    });

    pb.finish();

    if !errors.lock().unwrap().is_empty() {
        log_errors("validate_errors", errors)?;
        return Err(anyhow!(
            "Validation error: see 'validate_errors.json' file for details"
        ));
    }

    let message = "Validation complete, your metadata file(s) look good.";
    info!("{message}");
    println!("\n{message}");

    Ok(())
}
