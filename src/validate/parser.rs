pub use mpl_token_metadata::state::{MAX_NAME_LENGTH, MAX_SYMBOL_LENGTH, MAX_URI_LENGTH};

use crate::validate::errors::ValidateParserError;

pub fn check_name(name: &str) -> Result<(), ValidateParserError> {
    if name.len() > MAX_NAME_LENGTH {
        return Err(ValidateParserError::NameTooLong);
    }
    Ok(())
}

pub fn check_symbol(symbol: &str) -> Result<(), ValidateParserError> {
    if symbol.len() > MAX_SYMBOL_LENGTH {
        return Err(ValidateParserError::SymbolTooLong);
    }
    Ok(())
}

pub fn check_url(url: &str) -> Result<(), ValidateParserError> {
    if url.len() > MAX_URI_LENGTH {
        return Err(ValidateParserError::UrlTooLong);
    }
    Ok(())
}

pub fn check_seller_fee_basis_points(
    seller_fee_basis_points: u16,
) -> Result<(), ValidateParserError> {
    if seller_fee_basis_points > 10000 {
        return Err(ValidateParserError::InvalidCreatorShare);
    }
    Ok(())
}
