[View code on GitHub](https://github.com/metaplex-foundation/sugar/.autodoc/docs/json/src/guard)

The `guard` module in the Sugar project is responsible for managing Candy Guard entities, which act as mint authorities for candy machines, controlling the creation of new tokens. The module is organized into five sub-modules: `add`, `remove`, `show`, `update`, and `withdraw`. Each sub-module contains functions and structures related to their respective operations.

1. **add**: This sub-module provides functionality for adding a Candy Guard to a candy machine. The `process_guard_add` function initializes or updates a Candy Guard with the guards configuration and wraps the candy machine with the Candy Guard, establishing it as the mint authority.

   Example usage:
   ```rust
   let args = GuardAddArgs { /* ... */ };
   process_guard_add(args)?;
   ```

2. **remove**: This sub-module allows for removing a Candy Guard from a candy machine. The `process_guard_remove` function creates a transaction to remove the Candy Guard as the mint authority of the candy machine, transferring the mint authority to the user's keypair.

   Example usage:
   ```rust
   let args = GuardRemoveArgs { /* ... */ };
   process_guard_remove(args)?;
   ```

3. **show**: This sub-module displays the configuration details of a Candy Guard. The `process_guard_show` function fetches the Candy Guard account and its data from the Solana network and prints the configuration details, including the base, bump, authority, and data fields.

   Example usage:
   ```rust
   let args = GuardShowArgs { /* ... */ };
   process_guard_show(args)?;
   ```

4. **update**: This sub-module enables updating the Candy Guard configuration. The `process_guard_update` function loads the configuration data from the provided config file and sends an update transaction to the Candy Guard program with the new configuration data.

   Example usage:
   ```rust
   let args = GuardUpdateArgs { /* ... */ };
   process_guard_update(args)?;
   ```

5. **withdraw**: This sub-module handles the withdrawal of funds from a Candy Guard account. The `process_guard_withdraw` function creates a transaction to withdraw funds from the Candy Guard account and updates the cache if necessary.

   Example usage:
   ```rust
   let args = GuardWithdrawArgs { /* ... */ };
   process_guard_withdraw(args)?;
   ```

The `guard` module serves as a single entry point for managing Candy Guard entities in the Sugar project, providing a clean and organized structure for developers to interact with and manipulate Candy Guards.
