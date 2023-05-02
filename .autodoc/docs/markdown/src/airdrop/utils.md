[View code on GitHub](https://github.com/metaplex-foundation/sugar/src/airdrop/utils.rs)

This code is responsible for handling the loading and writing of airdrop results and lists in the Sugar project. It provides three main functions: `write_airdrop_results`, `load_airdrop_results`, and `load_airdrop_list`.

`write_airdrop_results` takes a reference to an `AirDropResults` object and writes it to a JSON file named `airdrop_results.json`. This function is useful for saving the current state of airdrop results to a file, which can be loaded later.

```rust
pub fn write_airdrop_results(airdrop_results: &AirDropResults) -> Result<()>;
```

`load_airdrop_results` takes a mutable reference to an `AirDropTargets` object and returns an `AirDropResults` object. It reads the `airdrop_results.json` file, deserializes it into an `AirDropResults` object, and syncs the results with the provided `AirDropTargets` object. If the file does not exist, it returns a new `AirDropResults` object.

```rust
pub fn load_airdrop_results(airdrop_list: &mut AirDropTargets) -> Result<AirDropResults>;
```

`load_airdrop_list` takes a `String` representing the path to an airdrop list JSON file and returns an `AirDropTargets` object. It checks if the file exists, and if it does, it reads the file, deserializes it into an `AirDropTargets` object, and returns it. If the file does not exist or has an incorrect format, it returns an error.

```rust
pub fn load_airdrop_list(airdrop_list: String) -> Result<AirDropTargets>;
```

These functions are essential for managing airdrop data in the Sugar project, allowing the user to save and load airdrop results and lists, and ensuring that the data is in the correct format.
## Questions: 
 1. **Question**: What is the purpose of the `write_airdrop_results` function and what is the format of the output file?
   **Answer**: The `write_airdrop_results` function is used to write the airdrop results to a JSON file named `airdrop_results.json`. The output file is formatted using the `serde_json::to_writer_pretty` function, which means it will be in a human-readable format.

2. **Question**: How does the `load_airdrop_results` function handle the case when the `airdrop_results.json` file does not exist?
   **Answer**: If the `airdrop_results.json` file does not exist, the `load_airdrop_results` function returns an empty `AirDropResults` object by calling `AirDropResults::new()`.

3. **Question**: What is the purpose of the `load_airdrop_list` function and what is the expected format of the input file?
   **Answer**: The `load_airdrop_list` function is used to load the airdrop targets from a JSON file specified by the `airdrop_list` parameter. The input file is expected to be in JSON format, and the function will return an `AirDropTargets` object containing the parsed data.