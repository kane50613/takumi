//! WebAssembly bindings for takumi's paint tree.
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::mem::take;

use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{Serializer, from_value, to_value};
use takumi_bindings_common::{
  FontCacheOptions, FontStore, NeedsOutput, append_css,
  input::{Font, ImageSource, decode_images, with_pending_value},
  source::{read_source, split_emojis},
  source_cache::SourceCache,
  stylesheet,
};
use takumi_core::{
  paint_tree::{PaintTree, PaintTreeOptions, paint_tree, paint_tree_needs},
  resources::image::ResourceCache,
  style::{CssSource, FontFamily, Lang},
  viewport::{DEFAULT_DEVICE_PIXEL_RATIO, Viewport},
};
use wasm_bindgen::prelude::*;

fn map_error(error: impl core::fmt::Display) -> js_sys::Error {
  js_sys::Error::new(&error.to_string())
}

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = include_str!("./dts-header.d.ts");

#[wasm_bindgen]
extern "C" {
  /// A layout node, the `{ json }` text of one, or HTML markup to parse in its place.
  #[wasm_bindgen(typescript_type = "Node | string | { json: string }")]
  pub type NodeType;
  /// JavaScript type for font input (details object or raw buffer).
  #[wasm_bindgen(typescript_type = "FontInput")]
  pub type FontType;
  /// JavaScript object holding the font cache budget.
  #[wasm_bindgen(typescript_type = "FontCacheOptions")]
  pub type FontCacheOptionsType;
  /// JavaScript type for the families produced by `registerFont`.
  #[wasm_bindgen(typescript_type = "RegisteredFamily[]")]
  pub type RegisteredFamiliesType;
  /// JavaScript object representing render options.
  #[wasm_bindgen(typescript_type = "PaintOptions")]
  pub type RenderOptionsType;
  /// JavaScript object listing what a node tree needs fetched.
  #[wasm_bindgen(typescript_type = "RenderNeeds")]
  pub type RenderNeedsType;
  /// JavaScript list of fonts a render may load, described before their bytes arrive.
  #[wasm_bindgen(typescript_type = "PendingFont[]")]
  pub type PendingFontsType;
  /// JavaScript object representing a painted tree.
  #[wasm_bindgen(typescript_type = "RawPaintTree")]
  pub type RawPaintTreeType;
}

/// Options for [`Painter::paint`].
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RenderOptions {
  width: Option<u32>,
  height: Option<u32>,
  device_pixel_ratio: Option<f32>,
  images: Option<Vec<ImageSource>>,
  css: Option<Vec<CssSource>>,
  font_families: Option<Vec<String>>,
  lang: Option<String>,
  emoji: Option<String>,
}

/// A renderer holding registered fonts and a decoded-resource cache.
///
/// Every method takes `&self` and the fonts are copy-on-write, mirroring the
/// other wasm bindings: a panic mid-call can't leave the wasm-bindgen borrow
/// flag or the font store held.
#[wasm_bindgen]
pub struct Painter {
  fonts: FontStore,
  resource_cache: ResourceCache,
  sources: SourceCache,
}

impl Painter {
  fn load_font(
    &self,
    font: FontType,
    key: Option<&str>,
    pinned: bool,
  ) -> Result<RegisteredFamiliesType, js_sys::Error> {
    let font: Font = from_value(font.into()).map_err(map_error)?;
    let registered = self.fonts.register(font, key, pinned).map_err(map_error)?;

    Ok(to_value(&registered).map_err(map_error)?.unchecked_into())
  }

  /// Calls `run` with the paint options `node` and `options` describe.
  fn with_tree_options<T>(
    &self,
    node: NodeType,
    options: Option<RenderOptionsType>,
    pending_fonts: Option<PendingFontsType>,
    run: impl FnOnce(PaintTreeOptions<'_>) -> T,
  ) -> Result<T, js_sys::Error> {
    let mut options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();
    let (mut node, document_css) = read_source(node.into(), &self.sources)?;

    split_emojis(&mut node, options.emoji.as_deref())?;
    options.css = append_css(options.css.take(), document_css);
    let images = decode_images(
      &self.resource_cache,
      options.images.as_deref().unwrap_or_default(),
    )
    .map_err(map_error)?;
    let lang = options
      .lang
      .as_deref()
      .map(Lang::parse)
      .transpose()
      .map_err(map_error)?;
    let fonts = with_pending_value(
      self.fonts.for_render(options.font_families.as_deref()),
      pending_fonts.map(Into::into),
    )?;
    Ok(run(
      PaintTreeOptions::builder()
        .viewport(
          Viewport::new((options.width, options.height)).with_device_pixel_ratio(
            options
              .device_pixel_ratio
              .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
          ),
        )
        .fonts(&fonts)
        .node(node)
        .images(images)
        .stylesheet(stylesheet(&self.resource_cache, options.css, Vec::new()).map_err(map_error)?)
        .font_families(options.font_families.map(FontFamily::from_names))
        .lang(lang)
        .build(),
    ))
  }
}

#[wasm_bindgen]
impl Painter {
  /// Creates a renderer with the bundled last-resort fonts.
  #[wasm_bindgen(constructor)]
  pub fn new(options: Option<FontCacheOptionsType>) -> Result<Painter, js_sys::Error> {
    let options: FontCacheOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    Ok(Painter {
      fonts: FontStore::new(options.font_cache_max_bytes).map_err(map_error)?,
      resource_cache: ResourceCache::default(),
      sources: SourceCache::default(),
    })
  }

  /// Registers a font (raw bytes or a details object), returning the families
  /// it produced.
  #[wasm_bindgen(js_name = registerFont)]
  pub fn register_font(
    &self,
    font: FontType,
    key: Option<String>,
  ) -> Result<RegisteredFamiliesType, js_sys::Error> {
    self.load_font(font, key.as_deref(), true)
  }

  /// Registers a font a render's `fonts` list loaded under `key`, released for space once the
  /// renderer's font cache is past its budget.
  #[wasm_bindgen(js_name = cacheFont)]
  pub fn cache_font(
    &self,
    font: FontType,
    key: &str,
  ) -> Result<RegisteredFamiliesType, js_sys::Error> {
    self.load_font(font, Some(key), false)
  }

  /// The families a registration under `key` added, when the renderer still holds them; `pin`
  /// keeps them from being released for space.
  #[wasm_bindgen(js_name = cachedFonts)]
  pub fn cached_fonts(
    &self,
    key: &str,
    pin: bool,
  ) -> Result<Option<RegisteredFamiliesType>, js_sys::Error> {
    self
      .fonts
      .cached(key, pin)
      .map(|registered| Ok(to_value(&registered).map_err(map_error)?.unchecked_into()))
      .transpose()
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

  /// Lays out a node tree and returns what painting it would draw.
  pub fn paint(
    &self,
    node: NodeType,
    options: Option<RenderOptionsType>,
  ) -> Result<RenderedPaint, js_sys::Error> {
    let mut tree = self
      .with_tree_options(node, options, None, paint_tree)?
      .map_err(map_error)?;
    let raw = take(&mut tree.document)
      .serialize(&Serializer::json_compatible())
      .map_err(map_error)?;

    Ok(RenderedPaint { tree, raw })
  }

  /// Reads the images a node tree looks up by URL and the characters its text shapes, after the
  /// cascade.
  pub fn needs(
    &self,
    node: NodeType,
    options: Option<RenderOptionsType>,
    pending_fonts: Option<PendingFontsType>,
  ) -> Result<RenderNeedsType, js_sys::Error> {
    let needs = self.with_tree_options(node, options, pending_fonts, paint_tree_needs)?;

    Ok(
      to_value(&NeedsOutput::from(needs))
        .map_err(map_error)?
        .unchecked_into(),
    )
  }
}

/// A serialized tree, and the font files and run outlines it keeps in wasm memory.
#[wasm_bindgen]
pub struct RenderedPaint {
  tree: PaintTree,
  raw: JsValue,
}

#[wasm_bindgen]
impl RenderedPaint {
  /// The serialized tree.
  pub fn tree(&self) -> RawPaintTreeType {
    self.raw.clone().unchecked_into()
  }

  /// The file of the tree's font at `index`.
  #[wasm_bindgen(js_name = fontData)]
  pub fn font_data(&self, index: usize) -> Option<Vec<u8>> {
    self.tree.font_data(index).map(<[u8]>::to_vec)
  }

  /// The SVG path data of each glyph of run `run` of text node `node`, `null` for a glyph the
  /// run's color does not fill.
  #[wasm_bindgen(js_name = runOutlines, unchecked_return_type = "(string | null)[] | undefined")]
  pub fn run_outlines(&self, node: usize, run: usize) -> Option<Vec<JsValue>> {
    Some(
      self
        .tree
        .run_outlines(node, run)?
        .into_iter()
        .map(|outline| outline.map_or(JsValue::NULL, |outline| JsValue::from_str(&outline)))
        .collect(),
    )
  }
}
