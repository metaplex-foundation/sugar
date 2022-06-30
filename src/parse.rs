use anchor_client::ClientError;
use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use regex::Regex;
use solana_client::{client_error::ClientErrorKind, rpc_request::RpcError};
use solana_program::program_error::ProgramError;
use std::{env, fs::File, path::Path};

use crate::config::data::*;

pub fn parse_solana_config() -> Option<SolanaConfig> {
    let home = if cfg!(unix) {
        env::var_os("HOME").expect("Couldn't find UNIX home key.")
    } else if cfg!(windows) {
        let drive = env::var_os("HOMEDRIVE").expect("Couldn't find Windows home drive key.");
        let path = env::var_os("HOMEPATH").expect("Couldn't find Windows home path key.");
        Path::new(&drive).join(&path).as_os_str().to_owned()
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME").expect("Couldn't find MacOS home key.")
    } else {
        panic!("Unsupported OS!");
    };

    let config_path = Path::new(&home)
        .join(".config")
        .join("solana")
        .join("cli")
        .join("config.yml");

    let conf_file = match File::open(config_path) {
        Ok(f) => f,
        Err(_) => return None,
    };
    serde_yaml::from_reader(&conf_file).ok()
}

pub fn path_to_string(path: &Path) -> Result<String> {
    match path.to_str() {
        Some(s) => Ok(s.to_string()),
        None => Err(anyhow!("Couldn't convert path to string.")),
    }
}

pub fn parse_client_error(e: ClientError) -> String {
    match e {
        ClientError::AnchorError(e) => format!("AnchorError: {e}"),
        ClientError::LogParseError(e) => format!("LogParseError: {e}"),
        ClientError::ProgramError(e) => match e {
            ProgramError::Custom(code) => format!("Code: {}", code),
            _ => format!("ProgramError: {}", e),
        },
        ClientError::SolanaClientError(e) => match e.kind {
            ClientErrorKind::Custom(code) => format!("Code: {}", code),
            ClientErrorKind::RpcError(e) => match e {
                RpcError::RpcRequestError(e) => format!("RpcRequestError: {}", e),
                RpcError::RpcResponseError {
                    code: _,
                    message,
                    data: _,
                } => parse_rpc_response_message(message),
                RpcError::ParseError(e) => format!("ParseError: {e}"),
                _ => format!("RpcError: {}", e),
            },
            ClientErrorKind::TransactionError(e) => format!("Transaction {}", e),
            _ => format!("SolanaClientError: {}", e),
        },
        _ => format!("Unmatched ClientError{}", e),
    }
}

fn parse_rpc_response_message(msg: String) -> String {
    lazy_static! {
        static ref RE: Regex =
            Regex::new(r"0x([A-Za-z1-9]+)").expect("Failed to compile parse_client_error regex.");
    }

    let mat = RE.find(&msg).unwrap();
    msg[mat.start()..mat.end()].to_string()
}
