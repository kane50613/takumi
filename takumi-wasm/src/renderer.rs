//! The main renderer for Takumi image rendering engine.

use crate::{helper::map_error, model::*};
use base64::{Engine, prelude::BASE64_STANDARD};
use serde_wasm_bindgen::{from_value, to_value};
use std::{
  borrow::Cow,
  collections::{HashMap, HashSet},
  sync::Arc,
};
use takumi::{
  GlobalContext,
  layout::{DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport, node::NodeKind},
  parley::{FontWeight, fontique::FontInfoOverride},
  rendering::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame, ImageOutputFormat,
    RenderOptionsBuilder, SequentialSceneBuilder, encode_animated_gif, encode_animated_png,
    encode_animated_webp, measure_layout, render, render_sequence_animation, write_image,
  },
  resources::image::{ImageSource as LoadedImageSource, load_image_source_from_bytes},
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
  fn fetch_resources_map(
    &self,
    resources: Option<&[ImageSource]>,
  ) -> Result<HashMap<Arc<str>, Arc<LoadedImageSource>>, js_sys::Error> {
    resources
      .map(|resources| {
        resources
          .iter()
          .map(|source| {
            let image = load_image_source_from_bytes(&source.data).map_err(map_error)?;
            Ok((source.src.clone(), image))
          })
          .collect::<Result<_, js_sys::Error>>()
      })
      .transpose()
      .map(|resources| resources.unwrap_or_default())
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
    let fetched_resources = self.fetch_resources_map(options.fetched_resources.as_deref())?;
    let dithering = options.dithering.unwrap_or_default();

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
      .keyframes(options.keyframes.unwrap_or_default())
      .time_ms(options.time_ms.unwrap_or_default().max(0) as u64)
      .dithering(dithering)
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

    let fetched_resources = self.fetch_resources_map(options.fetched_resources.as_deref())?;

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
      .keyframes(options.keyframes.unwrap_or_default())
      .time_ms(options.time_ms.unwrap_or_default().max(0) as u64)
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

  /// Renders a sequential animation timeline into a buffer.
  #[wasm_bindgen(js_name = renderAnimation)]
  pub fn render_animation(&self, options: RenderAnimationOptionsType) -> Result<Vec<u8>, JsValue> {
    let RenderAnimationOptions {
      scenes,
      width,
      height,
      format,
      quality,
      fetched_resources,
      draw_debug_border,
      stylesheets,
      device_pixel_ratio,
      fps,
    } = from_value(options.into()).map_err(map_error)?;
    let fetched_resources = self.fetch_resources_map(fetched_resources.as_deref())?;

    if scenes.is_empty() {
      return Err(JsValue::from_str("Expected at least one animation scene"));
    }

    if fps == 0 {
      return Err(JsValue::from_str("Expected fps to be greater than 0"));
    }

    let viewport = Viewport {
      width: Some(width),
      height: Some(height),
      font_size: DEFAULT_FONT_SIZE,
      device_pixel_ratio: device_pixel_ratio.unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
    };
    let draw_debug_border = draw_debug_border.unwrap_or_default();
    let stylesheets = stylesheets.unwrap_or_default();
    let scene_options = scenes
      .into_iter()
      .map(|scene| {
        SequentialSceneBuilder::default()
          .duration_ms(scene.duration_ms)
          .options(
            RenderOptionsBuilder::default()
              .viewport(viewport)
              .fetched_resources(fetched_resources.clone())
              .stylesheets(stylesheets.clone())
              .node(scene.node)
              .global(&self.context)
              .draw_debug_border(draw_debug_border)
              .build()
              .map_err(|e| JsValue::from_str(&format!("Failed to build render options: {e}")))?,
          )
          .build()
          .map_err(|e| JsValue::from_str(&format!("Failed to build animation scene: {e}")))
      })
      .collect::<Result<Vec<_>, _>>()?;
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
    let fetched_resources = self.fetch_resources_map(options.fetched_resources.as_deref())?;
    let viewport = Viewport {
      width: Some(options.width),
      height: Some(options.height),
      font_size: DEFAULT_FONT_SIZE,
      device_pixel_ratio: options
        .device_pixel_ratio
        .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
    };
    let stylesheets = options.stylesheets.unwrap_or_default();
    let rendered_frames = frames
      .into_iter()
      .map(|frame| -> Result<AnimationFrame, JsValue> {
        let render_options = RenderOptionsBuilder::default()
          .viewport(viewport)
          .fetched_resources(fetched_resources.clone())
          .node(frame.node)
          .global(&self.context)
          .draw_debug_border(options.draw_debug_border.unwrap_or_default())
          .stylesheets(stylesheets.clone())
          .build()
          .map_err(|e| JsValue::from_str(&format!("Failed to build render options: {e}")))?;

        let image = render(render_options).map_err(map_error)?;
        Ok(AnimationFrame::new(image, frame.duration_ms))
      })
      .collect::<Result<Vec<_>, JsValue>>()?;

    self.encode_animation(rendered_frames, options.format, options.quality)
  }
}
