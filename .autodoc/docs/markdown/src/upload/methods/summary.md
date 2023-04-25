[View code on GitHub](https://github.com/metaplex-foundation/sugar/.autodoc/docs/json/src/upload/methods)

The code in this folder is responsible for handling the upload of assets to various storage services and platforms. It provides a modular approach to uploading assets, allowing the larger project to easily integrate with different storage providers. The folder contains five main files, each representing a different storage method: `aws.rs`, `bundlr.rs`, `nft_storage.rs`, `pinata.rs`, and `shdw.rs`. Additionally, there is a `mod.rs` file that serves as a module for exposing the functionality of these sub-modules to other parts of the project.

For example, the `aws.rs` file provides a module for uploading assets to Amazon S3. It defines an `AWSMethod` struct that implements the `Prepare` and `ParallelUploader` traits. The `send` function uploads the asset to the S3 bucket, and the `upload_asset` method spawns an asynchronous task to upload the asset using the `send` function. Here's a usage example:

```rust
let config_data = ConfigData::load("config.toml").await?;
let aws_method = AWSMethod::new(&config_data).await?;

let asset_info = AssetInfo {
    asset_id: "example_asset_id".to_string(),
    data_type: DataType::Image,
    content: "path/to/image.png".to_string(),
    content_type: "image/png".to_string(),
    name: "image.png".to_string(),
};

let upload_handle = aws_method.upload_asset(asset_info);
let (asset_id, uploaded_url) = upload_handle.await??;
```

Similarly, the `bundlr.rs` file provides a module for uploading assets to the Bundlr platform using the Solana blockchain. The `BundlrMethod` struct handles the upload process, including setting up the Bundlr client, funding the Bundlr address, and uploading the assets. An example usage is provided in the file summary.

The `nft_storage.rs` file handles uploading files to the NFT Storage service, while the `pinata.rs` file provides functionality for uploading files to the Pinata IPFS service. Both files define structs that implement the `Prepare` and `Uploader` or `ParallelUploader` traits, respectively. Example usages for these modules can be found in their respective file summaries.

Finally, the `shdw.rs` file is responsible for handling the storage and uploading of assets to the Shadow Drive, a decentralized storage solution. It provides a `SHDWMethod` struct that implements the `Prepare` and `ParallelUploader` traits. An example usage is provided in the file summary.

The `mod.rs` file serves as a module that re-exports the contents of each sub-module, making their functions and types available to other parts of the project without the need to explicitly import each sub-module. This allows for a clean and modular approach to integrating various storage services into the larger project.
