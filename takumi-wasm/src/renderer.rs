//! The main renderer for Takumi image rendering engine.

use std::{collections::HashMap, mem::take, sync::Arc};

use base64::{Engine, prelude::BASE64_STANDARD};
use serde_wasm_bindgen::{from_value, to_value};
use takumi_bindings_common::{
  FontStore, NeedsOutput, Scenes, append_css, css_or_stylesheets,
  input::{decode_images, retain_svg},
  options::Draw,
  source::read_source,
  source_cache::SourceCache,
  stylesheet,
};
use takumi_core::{
  Fonts,
  layout::{needs::RenderNeeds, node::Node},
  resources::image::{ImageSource as LoadedImageSource, ResourceCache},
};
use takumi_raster::{SequentialScene, measure, needs, render, write_animation, write_image};
use wasm_bindgen::prelude::*;

use crate::{helper::map_error, model::*};

/// The main renderer for Takumi image rendering engine.
///
/// Every method takes `&self` and the fonts are copy-on-write, mirroring the
/// napi bindings: a panic mid-call can't leave the wasm-bindgen borrow flag or
/// the font store held, which would otherwise fail all subsequent calls.
#[wasm_bindgen]
pub struct Renderer {
  fonts: FontStore,
  resource_cache: ResourceCache,
  sources: SourceCache,
}

fn raster_options<'fonts>(
  resource_cache: &'fonts ResourceCache,
  fonts: &'fonts Fonts,
  node: Node,
  mut options: RenderOptions,
  images: HashMap<Arc<str>, LoadedImageSource>,
) -> Result<takumi_raster::RenderOptions<'fonts>, js_sys::Error> {
  let stylesheet = stylesheet(
    resource_cache,
    css_or_stylesheets(options.css.take(), options.stylesheets.take()),
    options.keyframes.take().unwrap_or_default(),
  )
  .map_err(map_error)?;

  options
    .raster(
      node,
      Draw {
        fonts,
        cache: resource_cache,
        stylesheet,
        images,
      },
    )
    .map_err(map_error)
}

#[wasm_bindgen]
impl Renderer {
  fn images_map(
    &self,
    images: Option<&[ImageSource]>,
  ) -> Result<HashMap<Arc<str>, LoadedImageSource>, js_sys::Error> {
    decode_images(&self.resource_cache, images.unwrap_or_default()).map_err(map_error)
  }

  /// Creates a new Renderer instance.
  #[wasm_bindgen(constructor)]
  pub fn new(options: Option<RendererOptionsType>) -> Result<Renderer, js_sys::Error> {
    let options: RendererOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    Ok(Renderer {
      fonts: FontStore::new(options.font_cache_bytes()).map_err(map_error)?,
      resource_cache: match options.cache_bytes() {
        Some(bytes) => ResourceCache::new(bytes),
        None => ResourceCache::default(),
      },
      sources: SourceCache::default(),
    })
  }

  /// Registers fonts into the renderer, returning the families each font produced.
  #[wasm_bindgen(js_name = registerFont)]
  pub fn register_font(
    &self,
    font: FontType,
    key: Option<String>,
  ) -> Result<RegisteredFamiliesType, js_sys::Error> {
    self.fonts.register_value(font, key.as_deref(), true)
  }

  /// Registers a font a render's `fonts` list loaded under `key`, released for space once the
  /// renderer's font cache is past its budget.
  #[wasm_bindgen(js_name = cacheFont)]
  pub fn cache_font(
    &self,
    font: FontType,
    key: &str,
  ) -> Result<RegisteredFamiliesType, js_sys::Error> {
    self.fonts.register_value(font, Some(key), false)
  }

  /// The families a registration under `key` added, when the renderer still holds them; `pin`
  /// keeps them from being released for space.
  #[wasm_bindgen(js_name = cachedFonts)]
  pub fn cached_fonts(
    &self,
    key: &str,
    pin: bool,
  ) -> Result<Option<RegisteredFamiliesType>, js_sys::Error> {
    self.fonts.cached_value(key, pin)
  }

  /// Removes every face registered under `family`, each `unicodeRange` subset alike, returning
  /// whether it had any.
  #[wasm_bindgen(js_name = unregisterFont)]
  pub fn unregister_font(&self, family: &str) -> bool {
    self.fonts.unregister_family(family)
  }

  /// Removes every registered font, leaving the bundled fallback font.
  #[wasm_bindgen(js_name = clearFonts)]
  pub fn clear_fonts(&self) -> Result<(), js_sys::Error> {
    self.fonts.reset().map_err(map_error)
  }

  /// Renders a node tree, or HTML, into an image buffer.
  #[wasm_bindgen(unchecked_return_type = "Uint8Array<ArrayBuffer>")]
  pub fn render(
    &self,
    source: SourceType,
    options: Option<RenderOptionsType>,
  ) -> Result<Vec<u8>, JsValue> {
    let (node, document_css) = read_source(source.into(), &self.sources)?;
    let mut options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    options.css = append_css(
      css_or_stylesheets(options.css.take(), options.stylesheets.take()),
      document_css,
    );

    let images = self.images_map(options.images.as_deref())?;
    let fonts = self.fonts.for_render(options.font_families.as_deref());
    self.render_internal(&fonts, node, options, images)
  }

  fn render_internal(
    &self,
    fonts: &Fonts,
    node: Node,
    options: RenderOptions,
    images: HashMap<Arc<str>, LoadedImageSource>,
  ) -> Result<Vec<u8>, JsValue> {
    let format = options
      .format
      .unwrap_or_default()
      .raster(options.quality, options.lossless);
    let render_options = raster_options(&self.resource_cache, fonts, node, options, images)?;
    let image = render(render_options).map_err(map_error)?;
    let Some(format) = format else {
      return Ok(image.into_raw());
    };
    let mut buffer = Vec::new();

    write_image(&image, &mut buffer, format).map_err(map_error)?;

    Ok(buffer)
  }

  /// Renders a node tree, or HTML, into an SVG document string.
  #[wasm_bindgen(js_name = renderSvg)]
  pub fn render_svg(
    &self,
    source: SourceType,
    options: Option<SvgRenderOptionsType>,
  ) -> Result<String, JsValue> {
    let (node, document_css) = read_source(source.into(), &self.sources)?;
    let mut options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    options.css = append_css(
      css_or_stylesheets(options.css.take(), options.stylesheets.take()),
      document_css,
    );

    let images = self.images_map(options.images.take().as_deref())?;
    let stylesheet = stylesheet(
      &self.resource_cache,
      options.css.take(),
      options.keyframes.take().unwrap_or_default(),
    )
    .map_err(map_error)?;
    let fonts = self.fonts.for_render(options.font_families.as_deref());
    let draw = Draw {
      fonts: &fonts,
      cache: &self.resource_cache,
      stylesheet,
      images,
    };
    let svg = takumi_svg::render(options.svg(node, draw).map_err(map_error)?).map_err(map_error)?;

    Ok(svg)
  }

  /// Measures a node tree, or HTML, and returns layout information.
  #[wasm_bindgen(js_name = measure)]
  pub fn measure(
    &self,
    source: SourceType,
    options: Option<RenderOptionsType>,
  ) -> Result<MeasuredNodeType, JsValue> {
    let (node, document_css) = read_source(source.into(), &self.sources)?;
    let mut options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    options.css = append_css(
      css_or_stylesheets(options.css.take(), options.stylesheets.take()),
      document_css,
    );

    let images = self.images_map(options.images.as_deref())?;

    let fonts = self.fonts.for_render(options.font_families.as_deref());
    let render_options = raster_options(&self.resource_cache, &fonts, node, options, images)?;

    let layout = measure(render_options).map_err(map_error)?;

    Ok(to_value(&layout).map_err(map_error)?.into())
  }

  /// Reads what a node tree, or HTML, needs fetched before it renders.
  #[wasm_bindgen(js_name = needs)]
  pub fn needs(
    &self,
    source: SourceType,
    options: Option<RenderOptionsType>,
    fonts: Option<ListedFontsType>,
  ) -> Result<RenderNeedsType, JsValue> {
    let listed = self.fonts.listed(fonts.map(Into::into))?;
    let (node, document_css) = read_source(source.into(), &self.sources)?;
    let mut options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    options.css = append_css(
      css_or_stylesheets(options.css.take(), options.stylesheets.take()),
      document_css,
    );

    retain_svg(&mut options.images);

    let images = self.images_map(options.images.take().as_deref())?;
    let fonts = self
      .fonts
      .for_needs(&mut options.font_families, listed.as_ref());
    let render_options = raster_options(&self.resource_cache, &fonts, node, options, images)?;

    Ok(
      to_value(&NeedsOutput::new(needs(render_options), listed.as_ref()))
        .map_err(map_error)?
        .into(),
    )
  }

  /// Renders a node tree, or HTML, into a data URL.
  ///
  /// `raw` format is not supported for data URL.
  #[wasm_bindgen(js_name = "renderAsDataUrl")]
  pub fn render_as_data_url(
    &self,
    source: SourceType,
    options: RenderOptionsType,
  ) -> Result<String, js_sys::Error> {
    let (node, document_css) = read_source(source.into(), &self.sources)?;
    let mut options: RenderOptions = from_value(options.into()).map_err(map_error)?;

    options.css = append_css(
      css_or_stylesheets(options.css.take(), options.stylesheets.take()),
      document_css,
    );

    let Some(format) = options.format.unwrap_or_default().raster(None, None) else {
      return Err(js_sys::Error::new(
        "Raw format is not supported for data URL",
      ));
    };

    let images = self.images_map(options.images.as_deref())?;
    let fonts = self.fonts.for_render(options.font_families.as_deref());
    let buffer = self.render_internal(&fonts, node, options, images)?;

    let mut data_uri = String::new();

    data_uri.push_str("data:");
    data_uri.push_str(format.content_type());
    data_uri.push_str(";base64,");
    data_uri.push_str(&BASE64_STANDARD.encode(buffer));

    Ok(data_uri)
  }

  /// Reads what every scene of an animation needs fetched before it renders.
  #[wasm_bindgen(js_name = animationNeeds)]
  pub fn animation_needs(
    &self,
    options: RenderAnimationOptionsType,
    fonts: Option<ListedFontsType>,
  ) -> Result<RenderNeedsType, JsValue> {
    let listed = self.fonts.listed(fonts.map(Into::into))?;
    let mut options: RenderAnimationOptions = from_value(options.into()).map_err(map_error)?;

    retain_svg(&mut options.images);

    let fonts = self
      .fonts
      .for_needs(&mut options.font_families, listed.as_ref());
    let needs = self
      .animation_scenes(&fonts, options)?
      .into_iter()
      .map(|(options, _)| needs(options))
      .collect::<RenderNeeds>();

    Ok(
      to_value(&NeedsOutput::new(needs, listed.as_ref()))
        .map_err(map_error)?
        .into(),
    )
  }

  /// Renders a sequential animation timeline into a buffer.
  #[wasm_bindgen(js_name = renderAnimation, unchecked_return_type = "Uint8Array<ArrayBuffer>")]
  pub fn render_animation(&self, options: RenderAnimationOptionsType) -> Result<Vec<u8>, JsValue> {
    let options: RenderAnimationOptions = from_value(options.into()).map_err(map_error)?;

    if let Some(reason) = options.invalid() {
      return Err(JsValue::from_str(reason));
    }

    let fps = options.fps;
    let format = options
      .format
      .unwrap_or_default()
      .raster(options.quality, options.lossless);
    let fonts = self.fonts.for_render(options.font_families.as_deref());
    let scenes = self
      .animation_scenes(&fonts, options)?
      .into_iter()
      .map(|(options, duration_ms)| {
        SequentialScene::builder()
          .duration_ms(duration_ms)
          .options(options)
          .build()
      })
      .collect::<Vec<_>>();

    let mut buffer = Vec::new();
    write_animation(&scenes, fps, format, &mut buffer).map_err(map_error)?;

    Ok(buffer)
  }
}

impl Renderer {
  /// Each scene's render options under `fonts` and how long it shows, its HTML parsed.
  fn animation_scenes<'fonts>(
    &'fonts self,
    fonts: &'fonts Fonts,
    mut options: RenderAnimationOptions,
  ) -> Result<Vec<(takumi_raster::RenderOptions<'fonts>, u32)>, JsValue> {
    let images = self.images_map(options.images.take().as_deref())?;
    let Scenes { nodes, css } = Scenes::read(
      take(&mut options.scenes)
        .into_iter()
        .map(|scene| (scene.node, scene.duration_ms)),
      css_or_stylesheets(options.css.take(), options.stylesheets.take()),
      read_source,
    )?;
    let stylesheet = stylesheet(
      &self.resource_cache,
      css,
      options.keyframes.take().unwrap_or_default(),
    )
    .map_err(map_error)?;
    let draw = Draw {
      fonts,
      cache: &self.resource_cache,
      stylesheet,
      images,
    };

    Ok(options.frames(nodes, draw).map_err(map_error)?)
  }
}
