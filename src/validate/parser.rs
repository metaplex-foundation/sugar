use std::str::FromStr;

use anchor_lang::prelude::Pubkey;

use crate::{
    common::*,
    validate::{errors::ValidateParserError, Creator},
};

pub fn check_seller_fee_basis_points(
    seller_fee_basis_points: u16,
) -> Result<(), ValidateParserError> {
    if seller_fee_basis_points > 10000 {
        return Err(ValidateParserError::InvalidSellerFeeBasisPoints(
            seller_fee_basis_points,
        ));
    }
    Ok(())
}

pub fn check_creators_shares(creators: &[Creator]) -> Result<(), ValidateParserError> {
    let mut shares = 0;
    for creator in creators {
        shares += creator.share;
    }

    if shares != 100 {
        return Err(ValidateParserError::InvalidCreatorShare);
    }
    Ok(())
}

pub fn check_creators_addresses(creators: &[Creator]) -> Result<(), ValidateParserError> {
    for creator in creators {
        Pubkey::from_str(&creator.address)
            .map_err(|_| ValidateParserError::InvalidCreatorAddress(creator.address.clone()))?;
    }

    Ok(())
}

pub fn check_category(category: &str) -> Result<(), ValidateParserError> {
    if !VALID_CATEGORIES.contains(&category) {
        return Err(ValidateParserError::InvalidCategory(
            category.to_string(),
            format!("{:?}", VALID_CATEGORIES),
        ));
    }

    Ok(())
}
