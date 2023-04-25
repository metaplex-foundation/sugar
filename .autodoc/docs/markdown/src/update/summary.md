[View code on GitHub](https://github.com/metaplex-foundation/sugar/.autodoc/docs/json/src/update)

The `update` module in the `sugar` project provides functionality for updating the configuration of a Candy Machine, which is a part of a larger NFT (Non-Fungible Token) project. It consists of two sub-modules: `process` and `set_token_standard`. These sub-modules are responsible for processing updates to the Candy Machine and setting the token standard and rule set, respectively.

The `process` sub-module contains the `process_update(args: UpdateArgs)` function, which takes an `UpdateArgs` struct as input and returns a `Result<()>`. This function is responsible for updating the configuration of a Candy Machine, including its symbol, seller fee basis points, max supply, mutability, creators, hidden settings, config line settings, and items available. It performs several steps, such as setting up the sugar configuration and client, retrieving the configuration data, loading the Candy Machine state, creating a new `CandyMachineData` struct with the updated configuration, asserting the correct authority, and sending update transactions to the Candy Machine.

Example usage of the `process_update` function:

```rust
use sugar;

fn main() {
    let update_args = sugar::UpdateArgs {
        keypair: "path/to/keypair.json",
        rpc_url: "https://api.mainnet-beta.solana.com",
        cache: "path/to/cache.json",
        new_authority: None,
        config: "path/to/config.json",
        candy_machine: None,
    };

    let result = sugar::process_update(update_args);
}
```

The `set_token_standard` sub-module provides the `process_set_token_stardard(args: SetTokenStandardArgs)` function, which takes a `SetTokenStandardArgs` struct as input and updates the token standard and rule set for a candy machine. It validates the input arguments, loads the candy machine state, sets up the client and program, retrieves the candy machine state, finds the required accounts and PDAs, sets the token standard, creates a transaction request, and sends the transaction.

Example usage of the `process_set_token_stardard` function:

```rust
use sugar;

fn main() {
    let set_token_standard_args = sugar::SetTokenStandardArgs {
        keypair: "path/to/keypair.json",
        rpc_url: "https://api.mainnet-beta.solana.com",
        cache: "path/to/cache.json",
        token_standard: Some("ERC721"),
        rule_set: None,
        candy_machine: None,
    };

    let result = sugar::process_set_token_stardard(set_token_standard_args);
}
```

In summary, the `update` module in the `sugar` project provides functionality for updating the configuration of a Candy Machine and setting the token standard and rule set. This allows users to modify various settings and properties of the Candy Machine, as well as manage and customize its behavior in the larger NFT project.
