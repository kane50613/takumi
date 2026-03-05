//! The main renderer for Takumi image rendering engine.

use crate::{
  helper::map_error,
  model::{
    AnimationFrameSource, AnimationFrameSourceType, AnimationOutputFormat, AnyNode,
    ConstructRendererOptions, ConstructRendererOptionsType, Font, FontType, ImageCacheKey,
    ImageSource, ImageSourceType, MeasuredNodeType, OutputFormat, RenderAnimationOptions,
    RenderAnimationOptionsType, RenderOptions, RenderOptionsType,
  },
};
use base64::{Engine, prelude::BASE64_STANDARD};
use serde_wasm_bindgen::{from_value, to_value};
use std::{borrow::Cow, collections::HashSet};
use takumi::{
  GlobalContext,
  layout::{DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport, node::NodeKind},
  parley::{FontWeight, fontique::FontInfoOverride},
  rendering::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame, ImageOutputFormat,
    RenderOptionsBuilder, encode_animated_gif, encode_animated_png, encode_animated_webp,
    measure_layout, render, write_image,
  },
  resources::image::load_image_source_from_bytes,
};
use wasm_bindgen::prelude::*;
use xxhash_rust::xxh3::{Xxh3DefaultBuilder, xxh3_64};

/// The main renderer for Takumi image rendering engine.
#[wasm_bindgen]
#[derive(Default)]
pub struct Renderer {
  pub(crate) context: GlobalContext,
  pub(crate) persistent_image_cache: HashSet<ImageCacheKey, Xxh3DefaultBuilder>,
}

#[wasm_bindgen]
impl Renderer {
  /// Creates a new Renderer instance.
  #[wasm_bindgen(constructor)]
  pub fn new(options: Option<ConstructRendererOptionsType>) -> Result<Renderer, js_sys::Error> {
    let options: ConstructRendererOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let mut renderer = Self::default();

    if let Some(fonts) = options.fonts {
      for font in fonts {
        renderer.load_font_internal(font)?;
      }
    }

    if let Some(images) = options.persistent_images {
      for image in images {
        renderer.put_persistent_image_internal(&image)?;
      }
    }

    Ok(renderer)
  }

  /// @deprecated use `loadFont` instead.
  #[wasm_bindgen(js_name = loadFontWithInfo)]
  pub fn load_font_with_info(&mut self, font: FontType) -> Result<(), js_sys::Error> {
    self.load_font(font)
  }

  fn load_font_internal(&mut self, font: Font) -> Result<(), js_sys::Error> {
    match font {
      Font::Buffer(buffer) => {
        self
          .context
          .font_context
          .load_and_store(buffer.into_vec().into(), None, None)
          .map_err(map_error)?;
      }
      Font::Object(details) => {
        self
          .context
          .font_context
          .load_and_store(
            details.data.into_vec().into(),
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

  /// Loads a font into the renderer.
  #[wasm_bindgen(js_name = loadFont)]
  pub fn load_font(&mut self, font: FontType) -> Result<(), js_sys::Error> {
    let input: Font = from_value(font.into()).map_err(map_error)?;
    self.load_font_internal(input)
  }

  /// Puts a persistent image into the renderer's internal store (internal version without JS conversion).
  fn put_persistent_image_internal(&mut self, data: &ImageSource) -> Result<(), js_sys::Error> {
    let key = ImageCacheKey {
      src: data.src.as_ref().into(),
      data_hash: xxh3_64(&data.data),
    };

    if self.persistent_image_cache.contains(&key) {
      return Ok(());
    }

    self.persistent_image_cache.insert(key);

    let image = load_image_source_from_bytes(&data.data).map_err(map_error)?;
    self
      .context
      .persistent_image_store
      .insert(data.src.to_string(), image);

    Ok(())
  }

  /// Puts a persistent image into the renderer's internal store.
  #[wasm_bindgen(js_name = putPersistentImage)]
  pub fn put_persistent_image(&mut self, data: ImageSourceType) -> Result<(), js_sys::Error> {
    let data: ImageSource = from_value(data.into()).map_err(map_error)?;
    self.put_persistent_image_internal(&data)
  }

  /// Clears the renderer's internal image store.
  #[wasm_bindgen(js_name = clearImageStore)]
  pub fn clear_image_store(&self) {
    self.context.persistent_image_store.clear();
  }

  /// Renders a node tree into an image buffer.
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
      .stylesheets(options.stylesheets.unwrap_or_default())
      .node(node)
      .global(&self.context)
      .build()
      .map_err(|e| JsValue::from_str(&format!("Failed to build render options: {e}")))?;

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
      .stylesheets(options.stylesheets.unwrap_or_default())
      .node(node)
      .global(&self.context)
      .build()
      .map_err(|e| JsValue::from_str(&format!("Failed to build render options: {e}")))?;

    let layout = measure_layout(render_options).map_err(map_error)?;

    Ok(to_value(&layout).map_err(map_error)?.into())
  }

  /// Renders a node tree into a data URL.
  ///
  /// `raw` format is not supported for data URL.
  #[wasm_bindgen(js_name = "renderAsDataUrl")]
  pub fn render_as_data_url(
    &self,
    node: AnyNode,
    options: RenderOptionsType,
  ) -> Result<String, js_sys::Error> {
    let node: NodeKind = from_value(node.into()).map_err(map_error)?;
    let options: RenderOptions = from_value(options.into()).map_err(map_error)?;

    let format = options.format.unwrap_or(OutputFormat::Png);

    if format == OutputFormat::Raw {
      return Err(js_sys::Error::new(
        "Raw format is not supported for data URL",
      ));
    }

    let buffer = self.render_internal(node, options)?;

    let mut data_uri = String::new();

    data_uri.push_str("data:");
    data_uri.push_str(ImageOutputFormat::from(format).content_type());
    data_uri.push_str(";base64,");
    data_uri.push_str(&BASE64_STANDARD.encode(buffer));

    Ok(data_uri)
  }

  /// Renders an animation sequence into a buffer.
  #[wasm_bindgen(js_name = renderAnimation)]
  pub fn render_animation(
    &self,
    frames: Vec<AnimationFrameSourceType>,
    options: RenderAnimationOptionsType,
  ) -> Result<Vec<u8>, JsValue> {
    let frames: Vec<AnimationFrameSource> = from_value(frames.into()).map_err(map_error)?;
    let options: RenderAnimationOptions = from_value(options.into()).map_err(map_error)?;

    let rendered_frames: Vec<AnimationFrame> = frames
      .into_iter()
      .map(|frame| -> Result<AnimationFrame, JsValue> {
        let render_options = RenderOptionsBuilder::default()
          .viewport((options.width, options.height).into())
          .node(frame.node)
          .global(&self.context)
          .draw_debug_border(options.draw_debug_border.unwrap_or_default())
          .build()
          .map_err(|e| JsValue::from_str(&format!("Failed to build render options: {e}")))?;

        let image = render(render_options).map_err(map_error)?;
        Ok(AnimationFrame::new(image, frame.duration_ms))
      })
      .collect::<Result<Vec<_>, JsValue>>()?;

    if let Some(quality) = options.quality
      && quality > 100
    {
      return Err(JsValue::from_str(&format!(
        "Invalid WebP quality {quality}; expected a value in 0..=100"
      )));
    }

    let mut buffer = Vec::new();

    match options.format.unwrap_or(AnimationOutputFormat::WebP) {
      AnimationOutputFormat::WebP => {
        let mut webp_options = AnimatedWebpOptions::default();
        if let Some(quality) = options.quality {
          webp_options.quality = quality;
        }

        encode_animated_webp(Cow::Owned(rendered_frames), &mut buffer, webp_options)
          .map_err(map_error)?;
      }
      AnimationOutputFormat::APng => {
        encode_animated_png(&rendered_frames, &mut buffer, AnimatedPngOptions::default())
          .map_err(map_error)?;
      }
      AnimationOutputFormat::Gif => {
        encode_animated_gif(
          Cow::Owned(rendered_frames),
          &mut buffer,
          AnimatedGifOptions::default(),
        )
        .map_err(map_error)?;
      }
    }

    Ok(buffer)
  }
}
