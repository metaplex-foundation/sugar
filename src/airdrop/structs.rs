use serde::{Deserialize, Serialize};

use crate::common::*;

pub type AirDropTargets = HashMap<String, u64>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransactionResult {
    pub signature: String,
    pub status: bool,
}

pub type AirDropResults = HashMap<String, Vec<TransactionResult>>;
