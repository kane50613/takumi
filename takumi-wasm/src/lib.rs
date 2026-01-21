#![deny(clippy::unwrap_used, clippy::expect_used)]
#![allow(
  clippy::module_name_repetitions,
  clippy::missing_errors_doc,
  clippy::missing_panics_doc,
  clippy::must_use_candidate
)]

use std::{fmt::Display, sync::Arc};

use base64::{Engine, prelude::BASE64_STANDARD};
use serde::Deserialize;
use serde_bytes::ByteBuf;
use serde_wasm_bindgen::{from_value, to_value};
use takumi::{
  GlobalContext,
  image::load_from_memory,
  layout::{
    DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport,
    node::{Node, NodeKind},
  },
  parley::{FontWeight, fontique::FontInfoOverride},
  rendering::{
    AnimationFrame, ImageOutputFormat, RenderOptionsBuilder, encode_animated_png,
    encode_animated_webp, measure_layout, render, write_image,
  },
  resources::{image::load_image_source_from_bytes, task::FetchTaskCollection},
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = r#"
export type AnyNode = { type: string; [key: string]: any };

export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

export type RenderOptions = {
  /**
   * The width of the image. If not provided, the width will be automatically calculated based on the content.
   */
  width?: number,
  /**
   * The height of the image. If not provided, the height will be automatically calculated based on the content.
   */
  height?: number,
  /**
   * The format of the image.
   * @default "png"
   */
  format?: "png" | "jpeg" | "webp",
  /**
   * The quality of JPEG format (0-100).
   */
  quality?: number,
  /**
   * The resources fetched externally. You should collect the fetch tasks first using `extractResourceUrls` and then pass the resources here.
   */
  fetchedResources?: ImageSource[],
  /**
   * Whether to draw debug borders.
   */
  drawDebugBorder?: boolean,
  /**
   * Defines the ratio resolution of the image to the physical pixels.
   * @default 1.0
   */
  devicePixelRatio?: number,
};

export type RenderAnimationOptions = {
  width: number,
  height: number,
  format?: "webp" | "apng",
  drawDebugBorder?: boolean,
};

export type FontDetails = {
  name?: string,
  data: ByteBuf,
  weight?: number,
  style?: "normal" | "italic" | "oblique",
};

export type ImageSource = {
  src: string,
  data: ByteBuf,
};

export type Font = FontDetails | ByteBuf;

export type MeasuredTextRun = {
  text: string,
  x: number,
  y: number,
  width: number,
  height: number,
};

export type MeasuredNode = {
  width: number,
  height: number,
  transform: [number, number, number, number, number, number],
  children: MeasuredNode[],
  runs: MeasuredTextRun[],
};
"#;

#[wasm_bindgen]
extern "C" {
  #[wasm_bindgen(typescript_type = "AnyNode")]
  #[derive(Debug)]
  pub type AnyNode;

  #[wasm_bindgen(typescript_type = "RenderOptions")]
  pub type RenderOptionsType;

  #[wasm_bindgen(typescript_type = "RenderAnimationOptions")]
  pub type RenderAnimationOptionsType;

  #[wasm_bindgen(typescript_type = "FontDetails")]
  pub type FontDetailsType;

  #[wasm_bindgen(typescript_type = "Font")]
  pub type FontType;

  #[wasm_bindgen(typescript_type = "ImageSource")]
  pub type ImageSourceType;

  #[wasm_bindgen(typescript_type = "MeasuredNode")]
  pub type MeasuredNodeType;
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RenderOptions {
  width: Option<u32>,
  height: Option<u32>,
  format: Option<ImageOutputFormat>,
  quality: Option<u8>,
  fetched_resources: Option<Vec<ImageSource>>,
  draw_debug_border: Option<bool>,
  device_pixel_ratio: Option<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderAnimationOptions {
  width: u32,
  height: u32,
  format: Option<AnimationOutputFormat>,
  draw_debug_border: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FontDetails {
  name: Option<String>,
  data: ByteBuf,
  weight: Option<f64>,
  style: Option<FontStyle>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Font {
  Object(FontDetails),
  Buffer(ByteBuf),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageSource {
  src: Arc<str>,
  data: ByteBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum AnimationOutputFormat {
  APng,
  WebP,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum FontStyle {
  Normal,
  Italic,
  Oblique,
}

#[wasm_bindgen]
pub struct AnimationFrameSource {
  node: AnyNode,
  #[wasm_bindgen(js_name = durationMs)]
  duration_ms: u32,
}

#[wasm_bindgen]
impl AnimationFrameSource {
  #[wasm_bindgen(constructor)]
  pub fn new(
    node: AnyNode,
    #[wasm_bindgen(js_name = durationMs)] duration_ms: u32,
  ) -> AnimationFrameSource {
    AnimationFrameSource { node, duration_ms }
  }
}

impl From<FontStyle> for takumi::parley::FontStyle {
  fn from(style: FontStyle) -> Self {
    match style {
      FontStyle::Italic => takumi::parley::FontStyle::Italic,
      FontStyle::Oblique => takumi::parley::FontStyle::Oblique(None),
      FontStyle::Normal => takumi::parley::FontStyle::Normal,
    }
  }
}

fn map_error<E: Display>(err: E) -> js_sys::Error {
  js_sys::Error::new(&err.to_string())
}

type JsResult<T> = Result<T, js_sys::Error>;

#[wasm_bindgen]
#[derive(Default)]
pub struct Renderer {
  context: GlobalContext,
}

#[wasm_bindgen]
impl Renderer {
  #[wasm_bindgen(constructor)]
  pub fn new() -> Renderer {
    Renderer::default()
  }

  /// @deprecated use `loadFont` instead.
  #[wasm_bindgen(js_name = loadFontWithInfo)]
  pub fn load_font_with_info(&mut self, font: FontType) -> JsResult<()> {
    self.load_font(font)
  }

  #[wasm_bindgen(js_name = loadFont)]
  pub fn load_font(&mut self, font: FontType) -> JsResult<()> {
    let input: Font = from_value(font.into()).map_err(map_error)?;

    match input {
      Font::Buffer(buffer) => {
        self
          .context
          .font_context
          .load_and_store(&buffer, None, None)
          .map_err(map_error)?;
      }
      Font::Object(details) => {
        self
          .context
          .font_context
          .load_and_store(
            &details.data,
            Some(FontInfoOverride {
              family_name: details.name.as_deref(),
              style: details.style.map(Into::into),
              weight: details.weight.map(|weight| FontWeight::new(weight as f32)),
              axes: None,
              width: None,
            }),
            None,
          )
          .map_err(map_error)?;
      }
    }
    Ok(())
  }

  #[wasm_bindgen(js_name = putPersistentImage)]
  pub fn put_persistent_image(&self, data: ImageSourceType) -> JsResult<()> {
    let data: ImageSource = from_value(data.into()).map_err(map_error)?;

    let image = load_from_memory(&data.data)
      .map_err(map_error)?
      .into_rgba8();

    self.context.persistent_image_store.insert(
      data.src.to_string(),
      Arc::new(takumi::resources::image::ImageSource::Bitmap(image)),
    );
    Ok(())
  }

  #[wasm_bindgen(js_name = clearImageStore)]
  pub fn clear_image_store(&self) {
    self.context.persistent_image_store.clear();
  }

  #[wasm_bindgen]
  pub fn render(
    &self,
    node: AnyNode,
    options: Option<RenderOptionsType>,
  ) -> Result<Vec<u8>, JsValue> {
    let node: NodeKind = from_value(node.into()).map_err(map_error)?;
    let options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    self.render_internal(node, options)
  }

  fn render_internal(&self, node: NodeKind, options: RenderOptions) -> Result<Vec<u8>, JsValue> {
    let fetched_resources = options
      .fetched_resources
      .map(|resources| -> Result<_, JsValue> {
        resources
          .into_iter()
          .map(|source| {
            let image = load_image_source_from_bytes(&source.data).map_err(map_error)?;
            Ok((source.src, image))
          })
          .collect::<Result<_, JsValue>>()
      })
      .transpose()?
      .unwrap_or_default();

    let render_options = RenderOptionsBuilder::default()
      .viewport(Viewport {
        width: options.width,
        height: options.height,
        font_size: DEFAULT_FONT_SIZE,
        device_pixel_ratio: options
          .device_pixel_ratio
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      })
      .draw_debug_border(options.draw_debug_border.unwrap_or_default())
      .fetched_resources(fetched_resources)
      .node(node)
      .global(&self.context)
      .build()
      .map_err(|e| JsValue::from_str(&format!("Failed to build render options: {e}")))?;

    let image = render(render_options).map_err(map_error)?;

    let mut buffer = Vec::new();

    write_image(
      &image,
      &mut buffer,
      options.format.unwrap_or(ImageOutputFormat::Png),
      options.quality,
    )
    .map_err(map_error)?;

    Ok(buffer)
  }

  #[wasm_bindgen(js_name = measure)]
  pub fn measure(
    &self,
    node: AnyNode,
    options: Option<RenderOptionsType>,
  ) -> Result<MeasuredNodeType, JsValue> {
    let node: NodeKind = from_value(node.into()).map_err(map_error)?;
    let options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let fetched_resources = options
      .fetched_resources
      .map(|resources| -> Result<_, JsValue> {
        resources
          .into_iter()
          .map(|source| {
            let image = load_image_source_from_bytes(&source.data).map_err(map_error)?;
            Ok((source.src, image))
          })
          .collect::<Result<_, JsValue>>()
      })
      .transpose()?
      .unwrap_or_default();

    let render_options = RenderOptionsBuilder::default()
      .viewport(Viewport {
        width: options.width,
        height: options.height,
        font_size: DEFAULT_FONT_SIZE,
        device_pixel_ratio: options
          .device_pixel_ratio
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      })
      .draw_debug_border(options.draw_debug_border.unwrap_or_default())
      .fetched_resources(fetched_resources)
      .node(node)
      .global(&self.context)
      .build()
      .map_err(|e| JsValue::from_str(&format!("Failed to build render options: {e}")))?;

    let layout = measure_layout(render_options).map_err(map_error)?;

    Ok(to_value(&layout).map_err(map_error)?.into())
  }

  #[wasm_bindgen(js_name = "renderAsDataUrl")]
  pub fn render_as_data_url(&self, node: AnyNode, options: RenderOptionsType) -> JsResult<String> {
    let node: NodeKind = from_value(node.into()).map_err(map_error)?;
    let options: RenderOptions = from_value(options.into()).map_err(map_error)?;

    let format = options.format.unwrap_or(ImageOutputFormat::Png);
    let buffer = self.render_internal(node, options)?;

    let mut data_uri = String::new();

    data_uri.push_str("data:");
    data_uri.push_str(format.content_type());
    data_uri.push_str(";base64,");
    data_uri.push_str(&BASE64_STANDARD.encode(buffer));

    Ok(data_uri)
  }

  #[wasm_bindgen(js_name = renderAnimation)]
  pub fn render_animation(
    &self,
    frames: Vec<AnimationFrameSource>,
    options: RenderAnimationOptionsType,
  ) -> Result<Vec<u8>, JsValue> {
    let options: RenderAnimationOptions = from_value(options.into()).map_err(map_error)?;

    let rendered_frames: Vec<AnimationFrame> = frames
      .into_iter()
      .map(|frame| -> Result<AnimationFrame, JsValue> {
        let node: NodeKind = from_value(frame.node.into()).map_err(map_error)?;
        let duration_ms = frame.duration_ms;

        let render_options = RenderOptionsBuilder::default()
          .viewport((options.width, options.height).into())
          .node(node)
          .global(&self.context)
          .draw_debug_border(options.draw_debug_border.unwrap_or_default())
          .build()
          .map_err(|e| JsValue::from_str(&format!("Failed to build render options: {e}")))?;

        let image = render(render_options).map_err(map_error)?;
        Ok(AnimationFrame::new(image, duration_ms))
      })
      .collect::<Result<Vec<_>, JsValue>>()?;

    let mut buffer = Vec::new();

    match options.format.unwrap_or(AnimationOutputFormat::WebP) {
      AnimationOutputFormat::WebP => {
        encode_animated_webp(&rendered_frames, &mut buffer, true, false, None)
          .map_err(map_error)?;
      }
      AnimationOutputFormat::APng => {
        encode_animated_png(&rendered_frames, &mut buffer, None).map_err(map_error)?;
      }
    }

    Ok(buffer)
  }
}

/// Collects the fetch task urls from the node.
#[wasm_bindgen(js_name = extractResourceUrls)]
pub fn extract_resource_urls(node: AnyNode) -> JsResult<Vec<String>> {
  let node: NodeKind = from_value(node.into()).map_err(map_error)?;

  let mut collection = FetchTaskCollection::default();

  node.collect_fetch_tasks(&mut collection);
  node.collect_style_fetch_tasks(&mut collection);

  Ok(
    collection
      .into_inner()
      .iter()
      .map(|task| task.to_string())
      .collect(),
  )
}

/// Collects the fetch task urls from the node.
/// @deprecated Use `extractResourceUrls` instead.
#[wasm_bindgen(js_name = collectNodeFetchTasks)]
pub fn collect_node_fetch_tasks(node: AnyNode) -> JsResult<Vec<String>> {
  extract_resource_urls(node)
}
