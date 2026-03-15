use axum::{
  extract::{Query, State},
  http::StatusCode,
  response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::from_str;
use std::borrow::Cow;
use takumi::{
  layout::{Viewport, node::Node},
  rendering::{DitheringAlgorithm, ImageOutputFormat, RenderOptions, render, write_image},
};
use tokio::task::spawn_blocking;

use crate::{AxumResult, AxumState};

#[derive(Deserialize)]
pub struct GenerateImageQuery {
  pub format: Option<ImageOutputFormat>,
  pub quality: Option<u8>,
  pub dithering: Option<DitheringAlgorithm>,
  pub payload: String,
  pub draw_debug_border: Option<bool>,
  pub width: Option<u32>,
  pub height: Option<u32>,
}

pub async fn generate_image_handler(
  Query(query): Query<GenerateImageQuery>,
  State(state): AxumState,
) -> AxumResult<Response> {
  let root_node: Node = from_str(&query.payload).map_err(|err| {
    (
      StatusCode::BAD_REQUEST,
      format!("Failed to parse node: {err}"),
    )
  })?;

  let format = query.format.unwrap_or(ImageOutputFormat::WebP);

  let buffer = spawn_blocking(move || -> AxumResult<Vec<u8>> {
    let viewport = Viewport::new(query.width, query.height);
    let dithering = query.dithering.unwrap_or_default();
    let options = RenderOptions::builder()
      .viewport(viewport)
      .node(root_node)
      .global(&state.context)
      .draw_debug_border(query.draw_debug_border.unwrap_or(false))
      .dithering(dithering)
      .build();

    let image = render(options).map_err(|_| {
      (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to render image.".to_string(),
      )
    })?;

    let mut buffer = Vec::new();

    write_image(Cow::Owned(image), &mut buffer, format, query.quality).map_err(|_| {
      (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to write image.".to_string(),
      )
    })?;

    Ok(buffer)
  })
  .await
  .map_err(|_| {
    (
      StatusCode::INTERNAL_SERVER_ERROR,
      "Image generation task panicked.".to_string(),
    )
  })??;

  Ok(([("content-type", format.content_type())], buffer).into_response())
}
