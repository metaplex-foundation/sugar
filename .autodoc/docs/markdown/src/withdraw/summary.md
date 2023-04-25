[View code on GitHub](https://github.com/metaplex-foundation/sugar/.autodoc/docs/json/src/withdraw)

The `withdraw` folder in the `sugar` project is responsible for handling the withdrawal of funds from Candy Machines. It contains two files: `mod.rs` and `process.rs`.

`mod.rs` manages the `process` module and re-exports its contents for easier access by other parts of the project. It declares a public module named `process` and re-exports all the public items from the `process` module at the current module level. This simplifies the usage of the `process` module's functions and items, making the overall project structure more organized and efficient.

`process.rs` provides a high-level function `process_withdraw` that takes `WithdrawArgs` as input and performs the withdrawal operation. The `WithdrawArgs` structure contains information about the candy machine, keypair, RPC URL, and a flag to list the candy machines.

The `process_withdraw` function follows these steps:

1. Set up the connection: It initializes the connection to the Solana network using the provided keypair and RPC URL. It sets up the program and payer using the `setup_withdraw` function.

2. Retrieve data for listing/draining: Depending on the `list` flag and the presence of a candy machine ID, the function either lists all candy machines or drains a specific candy machine. If the `list` flag is set, it retrieves and displays the candy machines and their balances. If a candy machine ID is provided, it drains the funds from that candy machine using the `do_withdraw` function.

The `do_withdraw` function takes a program, candy machine ID, and payer as input and sends a withdrawal request to the Solana network. It uses the `nft_accounts::Withdraw` and `nft_instruction::Withdraw` structures to build the request.

Here's an example of how the `process_withdraw` function can be used:

```rust
let args = WithdrawArgs {
    candy_machine: Some("CandyMachineID".to_string()),
    keypair: Some("KeypairPath".to_string()),
    rpc_url: Some("RPC_URL".to_string()),
    list: false,
};

process_withdraw(args).unwrap();
```

This code snippet initializes the `WithdrawArgs` structure with a candy machine ID, keypair path, and RPC URL, and then calls the `process_withdraw` function to withdraw funds from the specified candy machine.
