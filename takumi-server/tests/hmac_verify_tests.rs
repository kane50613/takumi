use hmac::{Hmac, Mac};
use sha2::Sha256;

#[test]
#[cfg(feature = "hmac_verify")]
fn test_verify_payload_success() {
  use takumi_server::hmac_verify::*;

  let secret = b"secret";
  let query = HmacQuery {
    hash: "d3b19dff7172a21a942ebf543e1a1cc6b39e7b086313bc0c226352c512f36404".to_string(),
    timestamp: 1672531200,
    payload: "payload".to_string(),
  };

  assert!(verify_payload(&query, secret).is_ok());
}

#[test]
#[cfg(feature = "hmac_verify")]
fn test_verify_payload_invalid_hash() {
  use takumi_server::hmac_verify::*;

  let secret = b"secret";
  let query = HmacQuery {
    hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
    timestamp: 1672531200,
    payload: "payload".to_string(),
  };

  assert!(verify_payload(&query, secret).is_err());
  assert_eq!(
    verify_payload(&query, secret).unwrap_err(),
    ERROR_HMAC_VERIFICATION
  );
}

#[test]
#[cfg(feature = "hmac_verify")]
fn test_verify_payload_wrong_secret() {
  use takumi_server::hmac_verify::*;

  let secret = b"wrong_secret";
  let query = HmacQuery {
    hash: "d3b19dff7172a21a942ebf543e1a1cc6b39e7b086313bc0c226352c512f36404".to_string(),
    timestamp: 1672531200,
    payload: "payload".to_string(),
  };

  assert!(verify_payload(&query, secret).is_err());
  assert_eq!(
    verify_payload(&query, secret).unwrap_err(),
    ERROR_HMAC_VERIFICATION
  );
}

#[test]
#[cfg(feature = "hmac_verify")]
fn test_verify_payload_malformed_hex() {
  use takumi_server::hmac_verify::*;

  let secret = b"secret";
  let query = HmacQuery {
    hash: "not-a-valid-hex-string".to_string(),
    timestamp: 1672531200,
    payload: "payload".to_string(),
  };

  assert!(verify_payload(&query, secret).is_err());
  assert_eq!(
    verify_payload(&query, secret).unwrap_err(),
    ERROR_HASH_LENGTH
  );
}

#[test]
#[cfg(feature = "hmac_verify")]
fn test_verify_payload_empty_secret() {
  use hmac::KeyInit;
  use takumi_server::hmac_verify::*;

  let secret = b"";
  let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
  mac.update(b"payload");
  mac.update(b";");
  mac.update(b"1672531200");
  let result = mac.finalize();
  let expected_hash = hex::encode(result.into_bytes());

  let query = HmacQuery {
    hash: expected_hash,
    timestamp: 1672531200,
    payload: "payload".to_string(),
  };

  assert!(verify_payload(&query, secret).is_ok());
}

#[test]
#[cfg(feature = "hmac_verify")]
fn test_verify_payload_empty_payload() {
  use hmac::KeyInit;
  use takumi_server::hmac_verify::*;

  let secret = b"secret";
  let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
  mac.update(b"");
  mac.update(b";");
  mac.update(b"1672531200");
  let result = mac.finalize();
  let expected_hash = hex::encode(result.into_bytes());

  let query = HmacQuery {
    hash: expected_hash,
    timestamp: 1672531200,
    payload: "".to_string(),
  };

  assert!(verify_payload(&query, secret).is_ok());
}

#[test]
#[cfg(feature = "hmac_verify")]
fn test_verify_payload_different_timestamp() {
  use takumi_server::hmac_verify::*;

  let secret = b"secret";
  let query = HmacQuery {
    hash: "d3b19dff7172a21a942ebf543e1a1cc6b39e7b086313bc0c226352c512f36404".to_string(),
    timestamp: 1672531201, // Different timestamp
    payload: "payload".to_string(),
  };

  assert!(verify_payload(&query, secret).is_err());
  assert_eq!(
    verify_payload(&query, secret).unwrap_err(),
    ERROR_HMAC_VERIFICATION
  );
}

#[test]
#[cfg(feature = "hmac_verify")]
fn test_verify_payload_short_hash() {
  use takumi_server::hmac_verify::*;

  let secret = b"secret";
  let query = HmacQuery {
    hash: "abcd".to_string(), // Too short
    timestamp: 1672531200,
    payload: "payload".to_string(),
  };

  assert!(verify_payload(&query, secret).is_err());
  assert_eq!(
    verify_payload(&query, secret).unwrap_err(),
    ERROR_HASH_LENGTH
  );
}

#[test]
#[cfg(feature = "hmac_verify")]
fn test_verify_payload_case_sensitivity() {
  use takumi_server::hmac_verify::*;

  let secret = b"secret";
  let query = HmacQuery {
    hash: "D3B19DFF7172A21A942EBF543E1A1CC6B39E7B086313BC0C226352C512F36404".to_string(), // Uppercase
    timestamp: 1672531200,
    payload: "payload".to_string(),
  };

  assert!(verify_payload(&query, secret).is_ok());
}

#[test]
#[cfg(feature = "hmac_verify")]
fn test_verify_payload_hash_length_validation() {
  use takumi_server::hmac_verify::*;

  let secret = b"secret";

  // Test 63 characters (too short)
  let query_short = HmacQuery {
    hash: "a".repeat(63),
    timestamp: 1672531200,
    payload: "payload".to_string(),
  };
  assert!(verify_payload(&query_short, secret).is_err());
  assert_eq!(
    verify_payload(&query_short, secret).unwrap_err(),
    ERROR_HASH_LENGTH
  );

  // Test 65 characters (too long)
  let query_long = HmacQuery {
    hash: "a".repeat(65),
    timestamp: 1672531200,
    payload: "payload".to_string(),
  };
  assert!(verify_payload(&query_long, secret).is_err());
  assert_eq!(
    verify_payload(&query_long, secret).unwrap_err(),
    ERROR_HASH_LENGTH
  );
}

#[test]
#[cfg(feature = "hmac_verify")]
fn test_verify_payload_non_hex_characters() {
  use takumi_server::hmac_verify::*;

  let secret = b"secret";
  let query = HmacQuery {
    hash: "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".to_string(), // Invalid hex chars
    timestamp: 1672531200,
    payload: "payload".to_string(),
  };

  assert!(verify_payload(&query, secret).is_err());
  assert_eq!(
    verify_payload(&query, secret).unwrap_err(),
    ERROR_HASH_LENGTH
  );
}
