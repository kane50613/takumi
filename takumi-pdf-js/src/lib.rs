//! WebAssembly bindings for takumi-pdf.

#![deny(missing_docs)]
#![deny(
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic,
  clippy::unreachable,
  clippy::todo,
  clippy::unimplemented
)]

use std::fmt::Display;

use serde_wasm_bindgen::{from_value, to_value};
use takumi_bindings_common::{
  FontStore, NeedsOutput, pdf::PdfRenderOptions, registrations::Retention,
  renderer::RendererOptions, source::read_source, source_cache::SourceCache, warning::WarningLog,
};
use takumi_core::{layout::node::Node, resources::cache::Cache};
use wasm_bindgen::prelude::*;

/// The heap, grown a part of the memory at a time.
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
#[global_allocator]
static HEAP: takumi_bindings_common::heap::Heap = takumi_bindings_common::heap::Heap::new();

pub(crate) fn map_error(error: impl Display) -> js_sys::Error {
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
  #[wasm_bindgen(typescript_type = "Font")]
  pub type FontType;

  /// JavaScript object holding the renderer options.
  #[wasm_bindgen(typescript_type = "RendererOptions")]
  pub type RendererOptionsType;

  /// JavaScript object reporting what a renderer's caches hold.
  #[wasm_bindgen(typescript_type = "CacheStats")]
  pub type CacheStatsType;

  /// JavaScript type for the families produced by `registerFont`.
  #[wasm_bindgen(typescript_type = "RegisteredFamily[]")]
  pub type RegisteredFamiliesType;

  /// JavaScript object representing render options.
  #[wasm_bindgen(typescript_type = "PdfRenderOptions")]
  pub type PdfRenderOptionsType;

  /// JavaScript object representing measure options.
  #[wasm_bindgen(typescript_type = "MeasureOptions")]
  pub type MeasureOptionsType;

  /// JavaScript object representing a measured size.
  #[wasm_bindgen(typescript_type = "MeasuredSize")]
  pub type MeasuredSizeType;

  /// JavaScript object listing what a document needs fetched.
  #[wasm_bindgen(typescript_type = "RenderNeeds")]
  pub type RenderNeedsType;
  /// JavaScript list of a render's fonts, described before their bytes arrive.
  #[wasm_bindgen(typescript_type = "ListedFont[]")]
  pub type ListedFontsType;
}

/// The size returned by [`PdfRenderer::measure`].
#[derive(serde::Serialize)]
struct MeasuredSizeOutput {
  width: f32,
  height: f32,
}

/// A PDF renderer holding registered fonts and a decoded-resource cache.
///
/// Every method takes `&self` and the fonts are copy-on-write, mirroring the
/// other wasm bindings: a panic mid-call can't leave the wasm-bindgen borrow
/// flag or the font store held.
#[wasm_bindgen]
pub struct PdfRenderer {
  fonts: FontStore,
  cache: Cache,
  sources: SourceCache,
  bands: SourceCache,
}

impl PdfRenderer {
  /// The document `node` describes and its options, with HTML bands parsed and `<style>` CSS appended.
  fn read_document(
    &self,
    node: NodeType,
    options: Option<JsValue>,
  ) -> Result<(Node, PdfRenderOptions), js_sys::Error> {
    let mut options: PdfRenderOptions = options
      .map(|options| from_value(options).map_err(map_error))
      .transpose()?
      .unwrap_or_default();
    let (node, document_css) = read_source(node.into(), &self.sources)?;

    options.resolve_bands(document_css, |html| {
      read_source(JsValue::from_str(html), &self.bands)
    })?;

    Ok((node, options))
  }
}

#[wasm_bindgen]
impl PdfRenderer {
  /// Creates a renderer with the bundled last-resort fonts.
  #[wasm_bindgen(constructor)]
  pub fn new(options: Option<RendererOptionsType>) -> Result<PdfRenderer, js_sys::Error> {
    WarningLog::print_to_console();

    let options: RendererOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let cache = options.cache();

    Ok(PdfRenderer {
      fonts: FontStore::new(cache.clone()).map_err(map_error)?,
      cache,
      sources: SourceCache::default(),
      bands: SourceCache::default(),
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
    self
      .fonts
      .register_value(font, key.as_deref(), Retention::Pinned, None)
  }

  /// Registers a font a render's `fonts` list loaded under `key`, its file leased to `lease` and
  /// evicted once no lease holds it and the renderer's cache needs the room.
  #[wasm_bindgen(js_name = cacheFont)]
  pub fn cache_font(
    &self,
    font: FontType,
    key: &str,
    lease: Option<u32>,
  ) -> Result<RegisteredFamiliesType, js_sys::Error> {
    self
      .fonts
      .register_value(font, Some(key), Retention::Evictable, lease)
  }

  /// The families a registration under `key` added, while the renderer holds its file. Leases the
  /// file to `lease`, or with `pin` keeps it from eviction for good.
  #[wasm_bindgen(js_name = cachedFonts)]
  pub fn cached_fonts(
    &self,
    key: &str,
    pin: bool,
    lease: Option<u32>,
  ) -> Result<Option<RegisteredFamiliesType>, js_sys::Error> {
    self.fonts.cached_value(key, pin, lease)
  }

  /// Loads the `@font-face` file `url` names, as `needs` reported it, leased to `lease` and evicted
  /// once no lease holds it and the renderer's cache needs the room.
  #[wasm_bindgen(js_name = cacheFontFile)]
  pub fn cache_font_file(
    &self,
    url: &str,
    data: Vec<u8>,
    lease: Option<u32>,
  ) -> Result<(), js_sys::Error> {
    self
      .fonts
      .cache_font_file(url, data, lease)
      .map_err(map_error)
  }

  /// Whether the renderer still holds the `@font-face` file `url`, leasing it to `lease` when it
  /// does.
  #[wasm_bindgen(js_name = cachedFontFile)]
  pub fn cached_font_file(&self, url: &str, lease: u32) -> bool {
    self.fonts.cached_font_file(url, lease)
  }

  /// Opens a set of leases that keeps the font files a render uses from eviction until
  /// `releaseFonts`.
  #[wasm_bindgen(js_name = leaseFonts)]
  pub fn lease_fonts(&self) -> u32 {
    self.fonts.lease_fonts()
  }

  /// Releases the leases of the set `lease` names, evicting what no longer fits.
  #[wasm_bindgen(js_name = releaseFonts)]
  pub fn release_fonts(&self, lease: u32) {
    self.fonts.release_fonts(lease);
  }

  /// Forgets the fonts registered under `keys` whose files no render leases, as a font with
  /// `cache: "none"` asks once its render settles, returning those keys and the ones nothing is
  /// registered under.
  #[wasm_bindgen(js_name = dropFonts)]
  pub fn drop_fonts(&self, keys: Vec<String>) -> Vec<String> {
    self.fonts.drop_fonts(&keys)
  }

  /// What the renderer's caches hold and how their lookups went since it was created.
  #[wasm_bindgen(js_name = cacheStats)]
  pub fn cache_stats(&self) -> Result<CacheStatsType, js_sys::Error> {
    self.fonts.cache_stats_value()
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

  /// Renders a node tree to PDF bytes. Without options the output is paged A4;
  /// `viewport` renders a single fixed page instead.
  #[wasm_bindgen(unchecked_return_type = "Uint8Array<ArrayBuffer>")]
  pub fn render(
    &self,
    node: NodeType,
    options: Option<PdfRenderOptionsType>,
  ) -> Result<Vec<u8>, js_sys::Error> {
    let (node, options) = self.read_document(node, options.map(Into::into))?;
    let fonts = self.fonts.for_render(&options.font_scope);

    options.render(node, &fonts, &self.cache).map_err(map_error)
  }

  /// Lays out a node tree without rendering and returns its size in CSS px.
  /// Page options lay out at the full page width, like a header/footer band;
  /// `pageNumber` / `totalPages` hooks are filled with three-digit counters.
  #[wasm_bindgen]
  pub fn measure(
    &self,
    node: NodeType,
    options: Option<MeasureOptionsType>,
  ) -> Result<MeasuredSizeType, js_sys::Error> {
    let (node, mut options) = self.read_document(node, options.map(Into::into))?;
    let fonts = self
      .fonts
      .for_needs(&mut options.font_families, None, &options.font_scope);
    let measured = options
      .measure(node, fonts, &self.cache)
      .map_err(map_error)?;

    Ok(
      to_value(&MeasuredSizeOutput {
        width: measured.width,
        height: measured.height,
      })
      .map_err(map_error)?
      .unchecked_into(),
    )
  }

  /// Reads the images a document and its bands look up and the pending fonts their text loads.
  pub fn needs(
    &self,
    node: NodeType,
    options: Option<PdfRenderOptionsType>,
    fonts: Option<ListedFontsType>,
  ) -> Result<RenderNeedsType, js_sys::Error> {
    let listed = self.fonts.listed(fonts.map(Into::into))?;
    let (node, mut options) = self.read_document(node, options.map(Into::into))?;
    let fonts = self.fonts.for_needs(
      &mut options.font_families,
      listed.as_ref(),
      &options.font_scope,
    );
    let needs = options.needs(node, fonts, &self.cache).map_err(map_error)?;

    Ok(
      to_value(&NeedsOutput::new(needs, listed.as_ref()))
        .map_err(map_error)?
        .unchecked_into(),
    )
  }
}
