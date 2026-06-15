use axum::extract::Query;
use takumi::core::GlobalContext;
use takumi::paint::rendering::ImageOutputFormat;

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
      width: Some(1200),
      height: Some(630),
      dithering: Default::default(),
    }),
    state,
  )
  .await
  .unwrap();
  assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_generate_image_handler_ico_content_type() {
  const NODE: &str = r#"{
    "type": "container",
    "tw": "w-100 h-100"
  }"#;

  let state = create_state(Args::default(), GlobalContext::default());
  let response = generate_image_handler(
    Query(GenerateImageQuery {
      format: Some(ImageOutputFormat::Ico),
      quality: None,
      payload: NODE.to_owned(),
      draw_debug_border: Some(false),
      width: Some(128),
      height: Some(128),
      dithering: Default::default(),
    }),
    state,
  )
  .await
  .unwrap();

  assert_eq!(response.status(), 200);
  assert_eq!(
    response.headers().get("content-type").unwrap(),
    "image/x-icon"
  );
}
