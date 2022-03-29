use bundlr_sdk::{tags::Tag, Bundlr, SolanaSigner};
use data_encoding::HEXLOWER;
use glob::glob;
use regex::Regex;
use ring::digest::{Context, SHA256};
use serde_json;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, Read},
    sync::Arc,
};

use crate::common::*;
use crate::upload_assets::errors::*;
use crate::validate::format::Metadata;

pub struct UploadDataArgs<'a> {
    pub bundlr_client: Arc<Bundlr<SolanaSigner>>,
    pub assets_dir: &'a Path,
    pub extension_glob: &'a str,
    pub tags: Vec<Tag>,
    pub data_type: DataType,
}

#[derive(Debug, Clone)]
pub enum DataType {
    Media,
    Metadata,
}

#[derive(Debug, Clone)]
pub struct AssetPair {
    pub name: String,
    pub metadata: String,
    pub metadata_hash: String,
    pub media: String,
    pub media_hash: String,
}

impl AssetPair {
    pub fn into_cache_item(self) -> CacheItem {
        CacheItem {
            name: self.name,
            media_hash: self.media_hash,
            media_link: String::new(),
            metadata_hash: self.metadata_hash,
            metadata_link: String::new(),
            on_chain: false,
        }
    }
}

pub fn get_data_size(assets_dir: &Path, extension: &str) -> Result<u64> {
    let path = assets_dir
        .join(format!("*.{extension}"))
        .to_str()
        .unwrap()
        .to_string();
    let assets = glob(&path)?;

    let mut total_size = 0;

    for asset in assets {
        let asset_path = asset?;
        let size = std::fs::metadata(asset_path)?.len();
        total_size += size;
    }

    Ok(total_size)
}

pub fn get_media_extension(assets_dir: &str) -> Result<String> {
    let entries = fs::read_dir(assets_dir)?;

    let re = Regex::new(r".+\d+\.(\w+[^json|JSON])$").expect("Failed to create regex.");

    for entry in entries {
        let path = entry?.path();
        if let Some(captures) = re.captures(path.to_str().unwrap()) {
            let extension = captures.get(1).unwrap().as_str();
            return Ok(extension.to_string());
        }
    }

    Err(UploadAssetsError::GetExtensionError.into())
}

pub fn get_asset_pairs(assets_dir: &str) -> Result<HashMap<usize, AssetPair>> {
    // filters out directories and hidden files
    let files = fs::read_dir(assets_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            !entry.file_name().to_str().unwrap().starts_with('.')
                && entry.metadata().unwrap().is_file()
        });

    let num_files = files.count();
    let mut asset_pairs: HashMap<usize, AssetPair> = HashMap::new();

    // number of files should be even
    if num_files % 2 != 0 {
        return Err(UploadAssetsError::InvalidNumberOfFiles(num_files).into());
    }

    // TODO: should we enforce that all files have the same extension?
    let extension = get_media_extension(assets_dir)?;

    // iterate over asset pairs
    for i in 0..(num_files / 2) {
        let metadata_file = PathBuf::from(assets_dir)
            .join(format!("{i}.json"))
            .to_str()
            .unwrap()
            .to_string();
        let media_file = Path::new(assets_dir)
            .join(format!("{i}.{extension}"))
            .to_str()
            .unwrap()
            .to_string();

        let m = File::open(&metadata_file)?;
        let metadata: Metadata = serde_json::from_reader(m)?;
        let name = metadata.name.clone();

        let asset_pair = AssetPair {
            name,
            metadata: metadata_file.clone(),
            metadata_hash: encode(&metadata_file)?,
            media: media_file.clone(),
            media_hash: encode(&media_file)?,
        };

        asset_pairs.insert(i, asset_pair);
    }

    Ok(asset_pairs)
}

fn encode(file: &str) -> Result<String> {
    let input = File::open(file)?;
    let mut reader = BufReader::new(input);
    let mut context = Context::new(&SHA256);
    let mut buffer = [0; 1024];

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }

    Ok(HEXLOWER.encode(context.finish().as_ref()))
}

pub fn get_updated_metadata(asset_pair: &AssetPair, cache_item: &CacheItem) -> Result<String> {
    let mut metadata: Metadata = {
        let m = OpenOptions::new().read(true).open(&asset_pair.metadata)?;
        serde_json::from_reader(&m)?
    };

    metadata.image = cache_item.media_link.clone();
    metadata.properties.files[0].uri = cache_item.media_link.clone();
    Ok(serde_json::to_string(&metadata).unwrap())
}
