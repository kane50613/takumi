use axum::http::Request;
use tower::ServiceExt;

use takumi::GlobalContext;
use takumi_server::{Args, create_app, create_state};

#[test]
fn test_create_state() {
  let state = create_state(Args::default(), GlobalContext::default());
  #[cfg(feature = "hmac_verify")]
  assert!(state.hmac_key.is_none());
}

#[tokio::test]
#[cfg(feature = "hmac_verify")]
async fn test_generate_image_handler_with_hmac_verify() {
  use std::time::{SystemTime, UNIX_EPOCH};

  use axum::body::Body;
  use hmac::{Hmac, Mac};
  use query_string_builder::QueryString;
  use sha2::Sha256;

  let app = create_app(create_state(Args::default(), GlobalContext::default()));

  const NODE: &str = r#"{
    "type": "container",
    "tw": "w-100 h-100"
  }"#;

  let payload = NODE.to_owned();
  let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_secs();

  let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
  mac.update(payload.as_bytes());
  mac.update(b";");
  mac.update(timestamp.to_string().as_bytes());
  let hash = mac.finalize().into_bytes();

  let uri = QueryString::dynamic()
    .with_value("payload", payload)
    .with_value("timestamp", timestamp)
    .with_value("width", "1200")
    .with_value("height", "630")
    .with_value("hash", hex::encode(hash));

  let request = Request::builder()
    .uri(format!("/image{uri}"))
    .body(Body::empty())
    .unwrap();

  let response = app.oneshot(request).await.unwrap();
  assert_eq!(response.status(), 200);
  assert_eq!(
    response.headers().get("content-type").unwrap(),
    "image/webp"
  );
}
