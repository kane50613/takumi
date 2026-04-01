use axum::{
  extract::{Query, Request, State},
  http::StatusCode,
  middleware::Next,
  response::Response,
};
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::{AxumResult, AxumState};

type HmacSha256 = Hmac<Sha256>;

#[derive(Deserialize)]
pub struct HmacQuery {
  pub hash: String,
  pub timestamp: u64,
  pub payload: String,
}

pub async fn hmac_verify_middleware(
  State(state): AxumState,
  Query(query): Query<HmacQuery>,
  request: Request,
  next: Next,
) -> AxumResult<Response> {
  let Some(secret) = state.hmac_key.as_ref() else {
    return Ok(next.run(request).await);
  };

  if let Err(err) = verify_payload(&query, secret) {
    return Err((StatusCode::BAD_REQUEST, err.to_string()));
  }

  Ok(next.run(request).await)
}

pub const ERROR_HASH_LENGTH: &str = "Hash must be a 64-character hexadecimal string";
pub const ERROR_INVALID_HEX: &str = "Invalid hexadecimal hash";
pub const ERROR_HMAC_VERIFICATION: &str = "HMAC verification failed";
pub const ERROR_HMAC_INIT: &str = "Failed to initialize HMAC";

pub fn verify_payload(query: &HmacQuery, secret: &[u8]) -> Result<(), &'static str> {
  if query.hash.len() != 64 {
    return Err(ERROR_HASH_LENGTH);
  }

  let decoded_hash = hex::decode(&query.hash).map_err(|_| ERROR_INVALID_HEX)?;

  let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| ERROR_HMAC_INIT)?;
  mac.update(query.payload.as_bytes());
  mac.update(b";");
  mac.update(query.timestamp.to_string().as_bytes());

  mac
    .verify_slice(&decoded_hash)
    .map_err(|_| ERROR_HMAC_VERIFICATION)
}
