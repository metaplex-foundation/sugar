use thiserror::Error;

#[derive(Debug, Error)]
pub enum SigningError {
    #[error("Missing metadata link for cache item {0}")]
    SigningMintFailed(String),
}
