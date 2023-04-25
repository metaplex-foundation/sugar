[View code on GitHub](https://github.com/metaplex-foundation/sugar/.autodoc/docs/json/src/deploy)

The `deploy` folder in the Sugar project contains code responsible for deploying a candy machine on the Solana blockchain, which represents a collection of NFTs with specific attributes and rules. The folder is organized into several files and sub-modules, each handling a specific aspect of the deployment process.

The `collection.rs` file contains the `create_collection` function, which creates a new collection of NFTs within the Sugar project. It initializes a new mint account, creates an associated token account, mints a single token, creates a metadata account, and creates a master edition account for the collection. Here's an example usage:

```rust
let client = Client::new(...);
let candy_machine_pubkey = Pubkey::new(...);
let mut cache = Cache::new(...);
let config_data = ConfigData::new(...);

let (signature, collection_mint_pubkey) = create_collection(&client, candy_machine_pubkey, &mut cache, &config_data)?;
```

The `config_lines.rs` file is responsible for uploading configuration lines to a candy machine program. It contains functions like `generate_config_lines`, `upload_config_lines`, and `add_config_lines` that work together to generate, upload, and add configuration lines to the candy machine. Example usage:

```rust
let num_items = 100;
let cache_items = get_cache_items();
let data = get_candy_machine_data();
let config_lines = generate_config_lines(num_items, &cache_items, &data)?;
let sugar_config = Arc::new(get_sugar_config());
let candy_pubkey = get_candy_pubkey();
let mut cache = get_cache();
let interrupted = Arc::new(AtomicBool::new(false));
let errors = upload_config_lines(sugar_config, candy_pubkey, &mut cache, config_lines, interrupted).await?;
```

The `errors.rs` file defines a custom error type called `DeployError` for handling various error scenarios that may occur during the deployment process of a candy machine.

The `initialize.rs` file is responsible for creating and initializing a candy machine. It contains functions like `create_candy_machine_data` and `initialize_candy_machine` that set up the candy machine with the provided configuration. Example usage:

```rust
let config_data = ConfigData::from_file("config.json")?;
let cache = Cache::from_file("cache.json")?;
let candy_machine_data = create_candy_machine_data(&client, &config_data, &cache)?;
let candy_account = Keypair::new();
let collection_mint = Pubkey::new_unique();
let collection_update_authority = Pubkey::new_unique();
let program = Program::new(...);

initialize_candy_machine(
    &config_data,
    &candy_account,
    candy_machine_data,
    collection_mint,
    collection_update_authority,
    program,
)?;
```

The `mod.rs` file organizes the `deploy` folder into sub-modules and re-exports their public items for easier access.

The `process.rs` file contains the `process_deploy` function, which is responsible for deploying a candy machine on the Solana blockchain. It validates metadata information, sets up the Solana client, creates or loads a candy machine, and generates and uploads config lines for each item in the cache. Example usage:

```rust
let deploy_args = DeployArgs {
    config: "config.json".to_string(),
    cache: "cache.json".to_string(),
    keypair: Some("keypair.json".to_string()),
    rpc_url: Some("https://api.mainnet-beta.solana.com".to_string()),
    interrupted: Arc::new(AtomicBool::new(false)),
    collection_mint: None,
};

process_deploy(deploy_args).await?;
```

Overall, the `deploy` folder provides a comprehensive set of tools for deploying candy machines on the Solana blockchain, making it easier for developers to manage and process NFT collections within their applications.
