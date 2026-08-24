//! The main renderer for Takumi image rendering engine.

use std::{
  collections::HashMap,
  sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use base64::{Engine, prelude::BASE64_STANDARD};
use serde_wasm_bindgen::{from_value, to_value};
use takumi_bindings_common::{build_font_resource, default_fonts, stylesheet};
use takumi_core::{
  Fonts,
  layout::node::Node,
  resources::{
    font::{FontResource, RegisteredFamily},
    image::{ImageSource as LoadedImageSource, ResourceCache},
  },
  style::{FontFamily, Lang},
  viewport::{DEFAULT_DEVICE_PIXEL_RATIO, Viewport},
};
use takumi_raster::{
  AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFormat, SequentialScene,
  measure, render, write_animation, write_image,
};
use wasm_bindgen::prelude::*;

use crate::{helper::map_error, model::*};

/// The main renderer for Takumi image rendering engine.
///
/// State lives behind a lock and every method takes `&self`, mirroring the
/// napi bindings: a panic mid-call can't leave the wasm-bindgen borrow flag
/// permanently set, which would otherwise fail all subsequent calls.
#[wasm_bindgen]
pub struct Renderer {
  state: RwLock<Fonts>,
  resource_cache: ResourceCache,
}

fn load_font_internal(
  fonts: &mut Fonts,
  font: Font,
) -> Result<Vec<RegisteredFamily>, js_sys::Error> {
  match font {
    Font::Buffer(buffer) => fonts
      .register(FontResource::new(buffer.into_vec()))
      .map_err(map_error),
    Font::Object(details) => {
      let data = details.data.into_vec();
      let resource = build_font_resource(
        &data,
        details.name,
        details.weight.map(|weight| weight as f32),
        details.style.map(Into::into),
        details.subset_of,
        details.subset_rank,
        details.generic,
      )
      .map_err(map_error)?;

      fonts.register(resource).map_err(map_error)
    }
  }
}

fn parse_lang(lang: Option<String>) -> Result<Option<Lang>, js_sys::Error> {
  lang
    .as_deref()
    .map(Lang::parse)
    .transpose()
    .map_err(map_error)
}

fn raster_options<'fonts>(
  resource_cache: &ResourceCache,
  fonts: &'fonts Fonts,
  node: Node,
  options: RenderOptions,
  images: HashMap<Arc<str>, LoadedImageSource>,
) -> Result<takumi_raster::RenderOptions<'fonts>, js_sys::Error> {
  let stylesheet = stylesheet(
    resource_cache,
    options.stylesheets,
    options.keyframes.unwrap_or_default(),
    options.variables,
  );
  let lang = parse_lang(options.lang)?;

  Ok(
    takumi_raster::RenderOptions::builder()
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
      .dithering(options.dithering.unwrap_or_default())
      .node(node)
      .fonts(fonts)
      .font_families(options.font_families.map(FontFamily::from_names))
      .lang(lang)
      .build(),
  )
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
  fn images_map(
    &self,
    images: Option<&[ImageSource]>,
  ) -> Result<HashMap<Arc<str>, LoadedImageSource>, js_sys::Error> {
    let mut map = HashMap::new();

    for source in images.unwrap_or_default() {
      let mode = source.cache.unwrap_or_default();
      let image = self
        .resource_cache
        .get_or_decode(&source.data, mode)
        .map_err(map_error)?;

      map.insert(source.src.clone(), image);
    }

    Ok(map)
  }

  /// Creates a new Renderer instance.
  #[wasm_bindgen(constructor)]
  pub fn new(options: Option<RendererOptionsType>) -> Result<Renderer, js_sys::Error> {
    let options: RendererOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    Ok(Renderer {
      state: RwLock::new(default_fonts().map_err(map_error)?),
      resource_cache: match options.cache_max_bytes {
        Some(bytes) => ResourceCache::new(bytes),
        None => ResourceCache::default(),
      },
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
  #[wasm_bindgen(unchecked_return_type = "Uint8Array<ArrayBuffer>")]
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

    let images = self.images_map(options.images.as_deref())?;
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
    let format = options.format.unwrap_or(OutputFormat::Png);
    let quality = options.quality;
    let render_options = raster_options(&self.resource_cache, fonts, node, options, images)?;

    let image = render(render_options).map_err(map_error)?;

    if format == OutputFormat::Raw {
      return Ok(image.into_raw());
    }

    let mut buffer = Vec::new();

    write_image(
      &image,
      &mut buffer,
      format.into_image_output_format(quality),
    )
    .map_err(map_error)?;

    Ok(buffer)
  }

  /// Renders a node tree into an SVG document string.
  #[wasm_bindgen(js_name = renderSvg)]
  pub fn render_svg(
    &self,
    node: NodeType,
    options: Option<SvgRenderOptionsType>,
  ) -> Result<String, JsValue> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: SvgRenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let images = self.images_map(options.images.as_deref())?;
    let stylesheet = stylesheet(
      &self.resource_cache,
      options.stylesheets,
      options.keyframes.unwrap_or_default(),
      options.variables,
    );
    let state = self.read_state()?;

    let lang = parse_lang(options.lang)?;

    let svg = takumi_svg::render(
      takumi_svg::SvgOptions::builder()
        .viewport(Viewport::new((options.width, options.height)))
        .images(images)
        .stylesheet(stylesheet)
        .time_ms(options.time_ms.unwrap_or_default().max(0) as u64)
        .node(node)
        .fonts(&state)
        .font_families(options.font_families.map(FontFamily::from_names))
        .lang(lang)
        .build(),
    )
    .map_err(map_error)?;

    Ok(svg)
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

    let images = self.images_map(options.images.as_deref())?;

    let state = self.read_state()?;
    let render_options = raster_options(&self.resource_cache, &state, node, options, images)?;

    let layout = measure(render_options).map_err(map_error)?;

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

    let images = self.images_map(options.images.as_deref())?;
    let state = self.read_state()?;
    let buffer = self.render_internal(&state, node, options, images)?;

    let mut data_uri = String::new();

    data_uri.push_str("data:");
    data_uri.push_str(format.into_image_output_format(None).content_type());
    data_uri.push_str(";base64,");
    data_uri.push_str(&BASE64_STANDARD.encode(buffer));

    Ok(data_uri)
  }

  /// Renders a sequential animation timeline into a buffer.
  #[wasm_bindgen(js_name = renderAnimation, unchecked_return_type = "Uint8Array<ArrayBuffer>")]
  pub fn render_animation(&self, options: RenderAnimationOptionsType) -> Result<Vec<u8>, JsValue> {
    let RenderAnimationOptions {
      scenes,
      width,
      height,
      format,
      images,
      draw_debug_border,
      stylesheets,
      keyframes,
      variables,
      device_pixel_ratio,
      fps,
      font_families,
      lang,
    } = from_value(options.into()).map_err(map_error)?;

    let lang = parse_lang(lang)?;

    let images = self.images_map(images.as_deref())?;

    if scenes.is_empty() {
      return Err(JsValue::from_str("Expected at least one animation scene"));
    }

    if fps == 0 {
      return Err(JsValue::from_str("Expected fps to be greater than 0"));
    }

    let viewport = Viewport::new((width, height))
      .with_device_pixel_ratio(device_pixel_ratio.unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO));
    let draw_debug_border = draw_debug_border.unwrap_or_default();
    let stylesheet = stylesheet(
      &self.resource_cache,
      stylesheets,
      keyframes.unwrap_or_default(),
      variables,
    );
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
              .font_families(font_families.clone().map(FontFamily::from_names))
              .lang(lang)
              .draw_debug_border(draw_debug_border)
              .build(),
          )
          .build()
      })
      .collect::<Vec<_>>();

    // wasm `image-webp` is lossless-only, which the WebP option defaults to.
    let format = match format.unwrap_or(AnimationOutputFormat::WebP) {
      AnimationOutputFormat::WebP => AnimationFormat::WebP(AnimatedWebpOptions::default()),
      AnimationOutputFormat::APng => AnimationFormat::Apng(AnimatedPngOptions::default()),
      AnimationOutputFormat::Gif => AnimationFormat::Gif(AnimatedGifOptions::default()),
    };

    let mut buffer = Vec::new();
    write_animation(&scene_options, fps, format, &mut buffer).map_err(map_error)?;

    Ok(buffer)
  }
}
