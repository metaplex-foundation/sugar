use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::validate::{errors, parser};

use super::ValidateParserError;

#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub struct Metadata {
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub seller_fee_basis_points: u16,
    pub image: String,
    pub animation_url: Option<String>,
    pub external_url: Option<String>,
    pub attributes: Vec<Attribute>,
    pub properties: Property,
}

impl Metadata {
    pub fn validate(self) -> Result<(), ValidateParserError> {
        parser::check_name(&self.name)?;
        parser::check_symbol(&self.symbol)?;
        parser::check_url(&self.image)?;
        parser::check_seller_fee_basis_points(self.seller_fee_basis_points)?;

        Ok(())
    }

    pub fn validate_strict(self) -> Result<(), ValidateParserError> {
        if self.animation_url.is_none() {
            return Err(errors::ValidateParserError::MissingAnimationUrl);
        } else {
            parser::check_url(&self.animation_url.unwrap())?;
        }

        if self.external_url.is_none() {
            return Err(errors::ValidateParserError::MissingExternalUrl);
        } else {
            parser::check_url(&self.external_url.unwrap())?;
        }

        parser::check_name(&self.name)?;
        parser::check_symbol(&self.symbol)?;
        parser::check_url(&self.image)?;
        parser::check_seller_fee_basis_points(self.seller_fee_basis_points)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub struct Property {
    pub files: Vec<FileAttr>,
}

#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub struct Attribute {
    pub trait_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub struct FileAttr {
    pub uri: String,
    #[serde(rename = "type")]
    pub file_type: String,
}
