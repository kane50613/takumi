use axum::extract::Query;
use takumi::GlobalContext;

use takumi_server::{GenerateImageQuery, args::Args, create_state, generate_image_handler};

#[tokio::test]
async fn test_generate_image_handler() {
  const NODE: &str = r#"{
    "type": "container",
    "tw": "w-100 h-100"
  }"#;

  let state = create_state(Args::default(), GlobalContext::default());
  let response = generate_image_handler(
    Query(GenerateImageQuery {
      format: None,
      quality: None,
      payload: NODE.to_owned(),
      draw_debug_border: Some(false),
      width: 1200,
      height: 630,
    }),
    state,
  )
  .await
  .unwrap();
  assert_eq!(response.status(), 200);
}
