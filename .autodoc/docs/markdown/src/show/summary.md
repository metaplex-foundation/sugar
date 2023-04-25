[View code on GitHub](https://github.com/metaplex-foundation/sugar/.autodoc/docs/json/src/show)

The `show` module in the `sugar` project is responsible for displaying information about a candy machine, which is part of a larger NFT minting project. The main functionality is provided by the `process_show(args: ShowArgs)` function in the `process.rs` file.

The `process_show` function takes a `ShowArgs` struct as input, which contains information about the candy machine, such as its keypair, RPC URL, cache, and whether to display unminted tokens. The function sets up the client and program for the candy machine using the provided arguments, loads the candy machine state and data, and prints various information about the candy machine, such as its ID, authority, mint authority, collection mint, token standard, rule set, and other properties.

If the `unminted` flag is set in the `ShowArgs` struct, the function also retrieves and displays a list of unminted indices for the candy machine. This is useful for users who want to know which tokens have not yet been minted.

For example, to use the `process_show` function, you would first create a `ShowArgs` struct with the necessary information:

```rust
use sugar::show::process::ShowArgs;

let show_args = ShowArgs {
    keypair: "path/to/keypair.json",
    rpc_url: "https://api.mainnet-beta.solana.com",
    cache: "path/to/cache",
    unminted: true,
};
```

Then, you can call the `process_show` function with the `show_args` struct:

```rust
use sugar::show::process::process_show;

fn main() {
    match process_show(show_args) {
        Ok(_) => println!("Candy machine information displayed successfully."),
        Err(e) => eprintln!("Error displaying candy machine information: {}", e),
    }
}
```

The `print_with_style` function in the `process.rs` file is a helper function used to print key-value pairs with a consistent formatting style. It takes an indent string, a key string, and a value that implements the `Display` trait, and prints them with a specific formatting style.

In summary, the `show` module provides a way for users to view detailed information about a candy machine, which can be helpful for managing and understanding the state of their NFT minting project. The `process_show` function is the main entry point for this functionality, and the `print_with_style` function is a helper function for consistent formatting of the displayed information.
