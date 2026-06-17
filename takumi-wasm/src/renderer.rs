//! The main renderer for Takumi image rendering engine.

use crate::{helper::map_error, model::*};
use base64::{Engine, prelude::BASE64_STANDARD};
use parley::{FontWeight, GenericFamily, fontique::FontInfoOverride};
use serde_wasm_bindgen::{from_value, to_value};
use std::{
  borrow::Cow,
  collections::HashMap,
  sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};
use takumi_base::{
  Fonts,
  layout::{
    DEFAULT_DEVICE_PIXEL_RATIO, Viewport,
    node::Node,
    style::{KeyframesRule, StyleSheet},
  },
  resources::{
    font::FontResource, image::ImageSource as LoadedImageSource, image_cache::ImageCache,
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
#[derive(Default)]
pub struct Renderer {
  state: RwLock<Fonts>,
  image_cache: ImageCache,
}

fn load_default_fonts(fonts: &mut Fonts) -> Result<(), js_sys::Error> {
  for (font, family_name, generic_family) in EMBEDDED_FONTS {
    let resource = FontResource::new((*font).to_vec())
      .override_info(FontInfoOverride {
        family_name: Some(*family_name),
        ..Default::default()
      })
      .generic_family(*generic_family);

    fonts.register(resource).map_err(map_error)?;
  }

  Ok(())
}

fn load_font_internal(fonts: &mut Fonts, font: Font) -> Result<(), js_sys::Error> {
  match font {
    Font::Buffer(buffer) => {
      fonts
        .register(FontResource::new(buffer.into_vec()))
        .map_err(map_error)?;
    }
    Font::Object(details) => {
      fonts
        .register(
          FontResource::new(details.data.into_vec()).override_info(FontInfoOverride {
            family_name: details.name.as_deref(),
            style: details.style.map(Into::into),
            weight: details.weight.map(|weight| FontWeight::new(weight as f32)),
            axes: None,
            width: None,
          }),
        )
        .map_err(map_error)?;
    }
  }
  Ok(())
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
    resources
      .map(|resources| {
        resources
          .iter()
          .map(|source| {
            let image = self
              .image_cache
              .get_or_decode(&source.data)
              .map_err(map_error)?;
            Ok((source.src.clone(), image))
          })
          .collect::<Result<_, js_sys::Error>>()
      })
      .transpose()
      .map(Option::unwrap_or_default)
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

  /// Configures this renderer's decoded-font cache (on by default, 256 MiB).
  #[wasm_bindgen(js_name = configureFontCache)]
  pub fn configure_font_cache(&self, options: wasm_bindgen::JsValue) -> Result<(), js_sys::Error> {
    let options: crate::FontCacheOptions = from_value(options).map_err(map_error)?;
    if let Some(max_bytes) = options.max_bytes {
      self
        .read_state()?
        .decode_cache()
        .set_max_bytes(max_bytes.max(0.0) as usize);
    }
    Ok(())
  }

  /// Configures this renderer's decoded-image cache (on by default, 256 MiB).
  #[wasm_bindgen(js_name = configureImageCache)]
  pub fn configure_image_cache(&self, options: wasm_bindgen::JsValue) -> Result<(), js_sys::Error> {
    let options: crate::ImageCacheOptions = from_value(options).map_err(map_error)?;
    if let Some(max_bytes) = options.max_bytes {
      self.image_cache.set_max_bytes(max_bytes.max(0.0) as usize);
    }
    Ok(())
  }

  /// Creates a new Renderer instance.
  #[wasm_bindgen(constructor)]
  pub fn new(options: Option<ConstructRendererOptionsType>) -> Result<Renderer, js_sys::Error> {
    let options: ConstructRendererOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let mut fonts = Fonts::default();

    let should_load_default_fonts = options
      .load_default_fonts
      .unwrap_or_else(|| options.fonts.is_none());

    if should_load_default_fonts {
      load_default_fonts(&mut fonts)?;
    }

    if let Some(custom_fonts) = options.fonts {
      for font in custom_fonts {
        load_font_internal(&mut fonts, font)?;
      }
    }

    Ok(Renderer {
      state: RwLock::new(fonts),
      image_cache: ImageCache::default(),
    })
  }

  /// Loads fonts into the renderer.
  #[wasm_bindgen(js_name = loadFonts)]
  pub fn load_fonts(&self, fonts: FontsType) -> Result<(), js_sys::Error> {
    let fonts: Vec<Font> = from_value(fonts.into()).map_err(map_error)?;
    let mut state = self.write_state()?;
    for font in fonts {
      load_font_internal(&mut state, font)?;
    }
    Ok(())
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

    let state = self.read_state()?;
    self.render_internal(&state, node, options)
  }

  fn render_internal(
    &self,
    fonts: &Fonts,
    node: Node,
    options: RenderOptions,
  ) -> Result<Vec<u8>, JsValue> {
    let images = self.fetch_resources_map(options.images.as_deref())?;
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

    let state = self.read_state()?;
    let buffer = self.render_internal(&state, node, options)?;

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
