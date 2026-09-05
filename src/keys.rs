use std::path::Path;

use rand::RngExt;
use secrecy::SecretBox;
use thiserror::Error;

pub type SigningKey = SecretBox<[u8; 32]>;

#[derive(Error, Debug)]
pub enum ExtractError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("the keyfile was malformed: it must be a list of string-encoded hexadecimal bytes")]
    Malformed,
    #[error("the key must be 32 bytes, but it was {0}")]
    WrongSize(usize),
}

pub async fn extract_signing_key(path: impl AsRef<Path>) -> Result<SigningKey, ExtractError> {
    let s = tokio::fs::read_to_string(path).await?;
    let bytes = hex::decode(&s).map_err(|_| ExtractError::Malformed)?;
    let len = bytes.len();
    Ok(SigningKey::new(Box::new(
        bytes.try_into().map_err(|_| ExtractError::WrongSize(len))?,
    )))
}

pub fn generate_signing_key() -> SigningKey {
    let mut rng = rand::rng();
    SigningKey::new(Box::new(rng.random()))
}
