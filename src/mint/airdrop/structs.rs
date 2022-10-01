use serde::{Deserialize, Serialize};

use crate::common::*;

// #[derive(Clone, Debug, Deserialize, Serialize)]
pub type AirDropTargets = HashMap<String, u64>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransactionResult {
    pub signature: String,
    pub status: bool,
}

// #[derive(Clone, Debug, Deserialize, Serialize)]
pub type AirDropResults = HashMap<String, Vec<TransactionResult>>;
