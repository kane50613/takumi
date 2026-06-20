//! The main renderer for Takumi image rendering engine.

use crate::{helper::map_error, model::*};
use base64::{Engine, prelude::BASE64_STANDARD};
use parley::{FontWeight, GenericFamily, fontique::FontInfoOverride};
use serde_wasm_bindgen::{from_value, to_value};
use std::{
  borrow::Cow,
  collections::HashMap,
  sync::{Arc, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard},
};
use takumi_base::{
  Fonts,
  layout::{
    DEFAULT_DEVICE_PIXEL_RATIO, Viewport,
    node::Node,
    style::{KeyframesRule, StyleSheet},
  },
  resources::{
    font::{FontResource, RegisteredFamily},
    image::{ImageCache, ImageSource as LoadedImageSource},
  },
};
use takumi_raster::{
  AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame, ImageOutputFormat,
  SequentialScene, encode_animated_gif, encode_animated_png, encode_animated_webp, measure_layout,
  render, render_sequence_animation, write_image,
};
use wasm_bindgen::prelude::*;

const EMBEDDED_FONTS: &[(&[u8], &str, GenericFamily)] = &[(
  include_bytes!("../../assets/fonts/manrope/manrope-latin-wght-normal.woff2"),
  "Manrope",
  GenericFamily::SansSerif,
)];

/// The main renderer for Takumi image rendering engine.
///
/// State lives behind a lock and every method takes `&self`, mirroring the
/// napi bindings: a panic mid-call can't leave the wasm-bindgen borrow flag
/// permanently set, which would otherwise fail all subsequent calls.
#[wasm_bindgen]
pub struct Renderer {
  state: RwLock<Fonts>,
  image_cache: ImageCache,
}

static DEFAULT_FONTS: OnceLock<Fonts> = OnceLock::new();

/// Returns a clone of the process-wide default font set, decoding the embedded
/// fonts once and sharing the decoded blobs across every renderer.
fn default_fonts() -> Result<Fonts, js_sys::Error> {
  if let Some(fonts) = DEFAULT_FONTS.get() {
    return Ok(fonts.clone());
  }

  let mut fonts = Fonts::default();
  for (font, family_name, generic_family) in EMBEDDED_FONTS {
    let resource = FontResource::new(*font)
      .override_info(FontInfoOverride {
        family_name: Some(*family_name),
        ..Default::default()
      })
      .generic_family(*generic_family);

    drop(fonts.register(resource).map_err(map_error)?);
  }

  if DEFAULT_FONTS.set(fonts.clone()).is_err()
    && let Some(stored) = DEFAULT_FONTS.get()
  {
    return Ok(stored.clone());
  }

  Ok(fonts)
}

fn load_font_internal(
  fonts: &mut Fonts,
  font: Font,
) -> Result<Vec<RegisteredFamily>, js_sys::Error> {
  match font {
    Font::Buffer(buffer) => fonts
      .register(FontResource::new(buffer.into_vec()))
      .map_err(map_error),
    Font::Object(details) => fonts
      .register(
        FontResource::new(details.data.into_vec()).override_info(FontInfoOverride {
          family_name: details.name.as_deref(),
          style: details.style.map(Into::into),
          weight: details.weight.map(|weight| FontWeight::new(weight as f32)),
          axes: None,
          width: None,
        }),
      )
      .map_err(map_error),
  }
}

impl Renderer {
  fn read_state(&self) -> Result<RwLockReadGuard<'_, Fonts>, js_sys::Error> {
    self
      .state
      .try_read()
      .map_err(|error| js_sys::Error::new(&format!("Renderer state is locked: {error}")))
  }

  fn write_state(&self) -> Result<RwLockWriteGuard<'_, Fonts>, js_sys::Error> {
    self
      .state
      .try_write()
      .map_err(|error| js_sys::Error::new(&format!("Renderer state is locked: {error}")))
  }
}

#[wasm_bindgen]
impl Renderer {
  fn parse_stylesheet(
    &self,
    stylesheets: Option<Vec<String>>,
    keyframes: Vec<KeyframesRule>,
  ) -> Result<StyleSheet, JsValue> {
    let stylesheet = StyleSheet::parse_owned_list_loosy(stylesheets.unwrap_or_default());
    let mut stylesheet = stylesheet;
    stylesheet.extend_keyframes(keyframes);
    Ok(stylesheet)
  }

  fn fetch_resources_map(
    &self,
    resources: Option<&[ImageSource]>,
  ) -> Result<HashMap<Arc<str>, LoadedImageSource>, js_sys::Error> {
    let mut map = HashMap::new();

    for source in resources.unwrap_or_default() {
      let mode = source.cache.unwrap_or_default();
      let image = self
        .image_cache
        .get_or_decode(&source.data, mode.stores())
        .map_err(map_error)?;

      map.insert(source.src.clone(), image);
    }

    Ok(map)
  }

  fn encode_animation(
    &self,
    frames: Vec<AnimationFrame>,
    format: Option<AnimationOutputFormat>,
    quality: Option<u8>,
  ) -> Result<Vec<u8>, JsValue> {
    if let Some(quality) = quality
      && quality > 100
    {
      return Err(JsValue::from_str(&format!(
        "Invalid WebP quality {quality}; expected a value in 0..=100"
      )));
    }

    let mut buffer = Vec::new();

    match format.unwrap_or(AnimationOutputFormat::WebP) {
      AnimationOutputFormat::WebP => {
        let mut webp_options = AnimatedWebpOptions::default();
        if let Some(quality) = quality {
          webp_options.quality = quality;
        }

        encode_animated_webp(Cow::Owned(frames), &mut buffer, webp_options).map_err(map_error)?;
      }
      AnimationOutputFormat::APng => {
        encode_animated_png(&frames, &mut buffer, AnimatedPngOptions::default())
          .map_err(map_error)?;
      }
      AnimationOutputFormat::Gif => {
        encode_animated_gif(
          Cow::Owned(frames),
          &mut buffer,
          AnimatedGifOptions::default(),
        )
        .map_err(map_error)?;
      }
    }

    Ok(buffer)
  }

  /// Creates a new Renderer instance.
  #[wasm_bindgen(constructor)]
  pub fn new() -> Result<Renderer, js_sys::Error> {
    Ok(Renderer {
      state: RwLock::new(default_fonts()?),
      image_cache: ImageCache::default(),
    })
  }

  /// Registers fonts into the renderer, returning the families each font produced.
  #[wasm_bindgen(js_name = registerFont)]
  pub fn register_font(&self, font: FontType) -> Result<RegisteredFamiliesType, js_sys::Error> {
    let font: Font = from_value(font.into()).map_err(map_error)?;

    let mut state = self.write_state()?;
    let registered = load_font_internal(&mut state, font)?;

    Ok(to_value(&registered).map_err(map_error)?.unchecked_into())
  }

  /// Renders a node tree into an image buffer.
  #[wasm_bindgen]
  pub fn render(
    &self,
    node: NodeType,
    options: Option<RenderOptionsType>,
  ) -> Result<Vec<u8>, JsValue> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let images = self.fetch_resources_map(options.images.as_deref())?;
    let state = self.read_state()?;
    self.render_internal(&state, node, options, images)
  }

  fn render_internal(
    &self,
    fonts: &Fonts,
    node: Node,
    options: RenderOptions,
    images: HashMap<Arc<str>, LoadedImageSource>,
  ) -> Result<Vec<u8>, JsValue> {
    let dithering = options.dithering.unwrap_or_default();
    let stylesheet =
      self.parse_stylesheet(options.stylesheets, options.keyframes.unwrap_or_default())?;

    let render_options = takumi_raster::RenderOptions::builder()
      .viewport(
        Viewport::new((options.width, options.height)).with_device_pixel_ratio(
          options
            .device_pixel_ratio
            .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
        ),
      )
      .draw_debug_border(options.draw_debug_border.unwrap_or_default())
      .images(images)
      .stylesheet(stylesheet)
      .time_ms(options.time_ms.unwrap_or_default().max(0) as u64)
      .dithering(dithering)
      .node(node)
      .fonts(fonts)
      .font_families(options.font_families)
      .build();

    let image = render(render_options).map_err(map_error)?;

    let format = options.format.unwrap_or(OutputFormat::Png);

    if format == OutputFormat::Raw {
      return Ok(image.into_raw());
    }

    let mut buffer = Vec::new();

    write_image(
      Cow::Owned(image),
      &mut buffer,
      format.into(),
      options.quality,
    )
    .map_err(map_error)?;

    Ok(buffer)
  }

  /// Measures a node tree and returns layout information.
  #[wasm_bindgen(js_name = measure)]
  pub fn measure(
    &self,
    node: NodeType,
    options: Option<RenderOptionsType>,
  ) -> Result<MeasuredNodeType, JsValue> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let images = self.fetch_resources_map(options.images.as_deref())?;
    let stylesheet =
      self.parse_stylesheet(options.stylesheets, options.keyframes.unwrap_or_default())?;

    let state = self.read_state()?;
    let render_options = takumi_raster::RenderOptions::builder()
      .viewport(
        Viewport::new((options.width, options.height)).with_device_pixel_ratio(
          options
            .device_pixel_ratio
            .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
        ),
      )
      .draw_debug_border(options.draw_debug_border.unwrap_or_default())
      .images(images)
      .stylesheet(stylesheet)
      .time_ms(options.time_ms.unwrap_or_default().max(0) as u64)
      .node(node)
      .fonts(&state)
      .font_families(options.font_families)
      .build();

    let layout = measure_layout(render_options).map_err(map_error)?;

    Ok(to_value(&layout).map_err(map_error)?.into())
  }

  /// Renders a node tree into a data URL.
  ///
  /// `raw` format is not supported for data URL.
  #[wasm_bindgen(js_name = "renderAsDataUrl")]
  pub fn render_as_data_url(
    &self,
    node: NodeType,
    options: RenderOptionsType,
  ) -> Result<String, js_sys::Error> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: RenderOptions = from_value(options.into()).map_err(map_error)?;

    let format = options.format.unwrap_or(OutputFormat::Png);

    if format == OutputFormat::Raw {
      return Err(js_sys::Error::new(
        "Raw format is not supported for data URL",
      ));
    }

    let images = self.fetch_resources_map(options.images.as_deref())?;
    let state = self.read_state()?;
    let buffer = self.render_internal(&state, node, options, images)?;

    let mut data_uri = String::new();

    data_uri.push_str("data:");
    data_uri.push_str(ImageOutputFormat::from(format).content_type());
    data_uri.push_str(";base64,");
    data_uri.push_str(&BASE64_STANDARD.encode(buffer));

    Ok(data_uri)
  }

  /// Renders a sequential animation timeline into a buffer.
  #[wasm_bindgen(js_name = renderAnimation)]
  pub fn render_animation(&self, options: RenderAnimationOptionsType) -> Result<Vec<u8>, JsValue> {
    let RenderAnimationOptions {
      scenes,
      width,
      height,
      format,
      quality,
      images,
      draw_debug_border,
      stylesheets,
      device_pixel_ratio,
      fps,
      font_families,
    } = from_value(options.into()).map_err(map_error)?;
    let images = self.fetch_resources_map(images.as_deref())?;

    if scenes.is_empty() {
      return Err(JsValue::from_str("Expected at least one animation scene"));
    }

    if fps == 0 {
      return Err(JsValue::from_str("Expected fps to be greater than 0"));
    }

    let viewport = Viewport::new((width, height))
      .with_device_pixel_ratio(device_pixel_ratio.unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO));
    let draw_debug_border = draw_debug_border.unwrap_or_default();
    let stylesheet = StyleSheet::parse_owned_list_loosy(stylesheets.unwrap_or_default());
    let state = self.read_state()?;
    let scene_options = scenes
      .into_iter()
      .map(|scene| {
        SequentialScene::builder()
          .duration_ms(scene.duration_ms)
          .options(
            takumi_raster::RenderOptions::builder()
              .viewport(viewport)
              .images(images.clone())
              .stylesheet(stylesheet.clone())
              .node(scene.node)
              .fonts(&state)
              .font_families(font_families.clone())
              .draw_debug_border(draw_debug_border)
              .build(),
          )
          .build()
      })
      .collect::<Vec<_>>();
    let rendered_frames = render_sequence_animation(&scene_options, fps).map_err(map_error)?;

    self.encode_animation(rendered_frames, format, quality)
  }

  /// Encodes a precomputed frame sequence into an animated image buffer.
  #[wasm_bindgen(js_name = encodeFrames)]
  pub fn encode_frames(
    &self,
    frames: Vec<AnimationFrameSourceType>,
    options: EncodeFramesOptionsType,
  ) -> Result<Vec<u8>, JsValue> {
    let frames: Vec<AnimationFrameSource> = from_value(frames.into()).map_err(map_error)?;
    let options: EncodeFramesOptions = from_value(options.into()).map_err(map_error)?;
    let images = self.fetch_resources_map(options.images.as_deref())?;
    let viewport = Viewport::new((options.width, options.height)).with_device_pixel_ratio(
      options
        .device_pixel_ratio
        .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
    );
    let stylesheet = StyleSheet::parse_owned_list_loosy(options.stylesheets.unwrap_or_default());
    let state = self.read_state()?;
    let rendered_frames = frames
      .into_iter()
      .map(|frame| -> Result<AnimationFrame, JsValue> {
        let render_options = takumi_raster::RenderOptions::builder()
          .viewport(viewport)
          .images(images.clone())
          .node(frame.node)
          .fonts(&state)
          .font_families(options.font_families.clone())
          .draw_debug_border(options.draw_debug_border.unwrap_or_default())
          .stylesheet(stylesheet.clone())
          .build();

        let image = render(render_options).map_err(map_error)?;
        Ok(AnimationFrame::new(image, frame.duration_ms))
      })
      .collect::<Result<Vec<_>, JsValue>>()?;

    self.encode_animation(rendered_frames, options.format, options.quality)
  }
}
