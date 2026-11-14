//! Platform-agnostic glue shared by the napi and wasm bindings.
//!
//! Both bindings lower raw JS input into a takumi render request the same way —
//! the embedded fallback fonts, a font resource from optional fields, the
//! stylesheet, and the per-render options. That lowering lives here so neither binding re-derives it. Each
//! binding keeps only its platform-specific glue (JS type coercion, error
//! mapping, threading).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#[cfg(all(
  feature = "wasm",
  target_arch = "wasm32",
  not(target_feature = "atomics")
))]
pub mod heap;
pub mod input;
#[cfg(feature = "raster")]
pub mod options;
pub mod registrations;
pub mod renderer;
#[cfg(feature = "wasm")]
pub mod source;
pub mod source_cache;

#[cfg(feature = "wasm")]
use std::fmt::Display;
use std::{cell::RefCell, rc::Rc, sync::Arc};

use input::{Font, ListedFonts, register_font};
use registrations::{FontRegistrations, RenderLeases, Retention};
#[cfg(feature = "wasm")]
use renderer::{CacheOptions, CacheStats};
use serde::Serialize;
#[cfg(feature = "wasm")]
use serde_wasm_bindgen::{Serializer, from_value, to_value};
use source_cache::{ParsedSource, SourceCache};
#[cfg(feature = "wasm")]
use wasm_bindgen::{JsCast, JsValue};

use takumi_core::{
  Error as CoreError, Fonts,
  layout::{needs::RenderNeeds, node::Node},
  resources::{
    cache::Cache,
    font::{FontError, FontOverride, FontResource, FontSource, RegisteredFamily},
  },
  style::{CssSource, CssSourceError, KeyframesRule, Lang, StyleSheet},
  viewport::DEFAULT_DEVICE_PIXEL_RATIO,
};

/// Last-resort only: no generic family claim, so `sans-serif` and friends
/// resolve to caller-registered fonts via the fallback bucket instead of this
/// face.
const EMBEDDED_FONTS: &[(&[u8], &str)] = &[(
  include_bytes!("../../assets/fonts/geist/geist-latin-wght-300-800.ttf"),
  "Geist",
)];

/// The default font set holding the embedded last-resort fonts.
pub fn default_fonts() -> Result<Fonts, FontError> {
  let mut fonts = Fonts::default();

  register_embedded(&mut fonts, |_| true)?;

  Ok(fonts)
}

/// Registers the embedded last-resort fonts whose family `wanted` accepts.
fn register_embedded(fonts: &mut Fonts, wanted: impl Fn(&str) -> bool) -> Result<(), FontError> {
  for (bytes, name) in EMBEDDED_FONTS {
    if !wanted(name) {
      continue;
    }

    let resource = FontResource::new(FontSource::from_static(bytes))
      .override_info(FontOverride {
        family_name: Some((*name).into()),
        ..Default::default()
      })
      .last_resort();

    drop(fonts.register(resource)?);
  }

  Ok(())
}

/// Registered fonts for the single-threaded wasm bindings, copy-on-write like the napi
/// bindings' `ArcSwap`: a call takes the current registry and lets go of the store at once, and
/// a change publishes a new registry. Nothing is held across a call, so one that traps (release
/// builds abort on panic) leaves the store usable.
pub struct FontStore {
  fonts: RefCell<Rc<Fonts>>,
  registrations: RefCell<FontRegistrations>,
  cache: Cache,
  leases: RenderLeases,
}

impl FontStore {
  /// A store holding the embedded last-resort fonts, whose font files `cache` holds.
  pub fn new(cache: Cache) -> Result<Self, FontError> {
    Ok(Self {
      fonts: RefCell::new(Rc::new(default_fonts()?)),
      registrations: RefCell::default(),
      cache,
      leases: RenderLeases::default(),
    })
  }

  /// The registry for a render, with the `@font-face` files its document `failed` to download.
  pub fn for_render(&self, failed: Option<&[String]>) -> Rc<Fonts> {
    self.release_evicted();

    let failed = failed.unwrap_or_default();

    if failed.is_empty() {
      self.current()
    } else {
      Rc::new(self.current().with_failed_font_files(failed))
    }
  }

  /// The registry for a needs pass, with each listed font still loading registered as pending.
  pub fn for_needs(
    &self,
    families: &mut Option<Vec<String>>,
    listed: Option<&ListedFonts>,
    failed: Option<&[String]>,
  ) -> Rc<Fonts> {
    let Some(listed) = listed else {
      return self.for_render(failed);
    };

    if families.is_none() {
      families.clone_from(&listed.families);
    }

    Rc::new(listed.fonts(&self.for_render(failed)))
  }

  /// Registers the font JavaScript describes under `key`, its file leased to `lease`, kept as
  /// `retention` says.
  #[cfg(feature = "wasm")]
  pub fn register_value<T: JsCast>(
    &self,
    font: impl Into<JsValue>,
    key: Option<&str>,
    retention: Retention,
    lease: Option<u32>,
  ) -> Result<T, js_sys::Error> {
    let font = Font::from_js(font.into()).map_err(js_error)?;
    let registered = self
      .register(font, key, retention, lease)
      .map_err(js_error)?;

    Ok(to_value(&registered).map_err(js_error)?.unchecked_into())
  }

  /// [`Self::cached`] as the binding's JavaScript type `T`.
  #[cfg(feature = "wasm")]
  pub fn cached_value<T: JsCast>(
    &self,
    key: &str,
    pin: bool,
    lease: Option<u32>,
  ) -> Result<Option<T>, js_sys::Error> {
    self
      .cached(key, pin, lease)
      .map(|registered| Ok(to_value(&registered).map_err(js_error)?.unchecked_into()))
      .transpose()
  }

  /// A render's `fonts` list, as JavaScript describes it, read against the fonts this store holds.
  #[cfg(feature = "wasm")]
  pub fn listed(&self, listed: Option<JsValue>) -> Result<Option<ListedFonts>, js_sys::Error> {
    self.release_evicted();

    listed
      .map(|listed| {
        from_value(listed)
          .map(|listed| ListedFonts::read(&self.registrations.borrow(), listed))
          .map_err(js_error)
      })
      .transpose()
  }

  /// Applies the cache options JavaScript describes to the cache, evicting what no longer fits.
  #[cfg(feature = "wasm")]
  pub fn set_cache_options_value(&self, options: JsValue) -> Result<(), js_sys::Error> {
    let options: CacheOptions = from_value(options).map_err(js_error)?;

    self.cache.set_options(options.pool());
    self.release_evicted();

    Ok(())
  }

  /// What the cache holds, as the binding's JavaScript type `T`.
  #[cfg(feature = "wasm")]
  pub fn cache_stats_value<T: JsCast>(&self) -> Result<T, js_sys::Error> {
    let stats = CacheStats::new(&self.cache)
      .serialize(&Serializer::new().serialize_missing_as_null(true))
      .map_err(js_error)?;

    Ok(stats.unchecked_into())
  }

  fn current(&self) -> Rc<Fonts> {
    self.fonts.borrow().clone()
  }

  /// Applies `change` to a copy of the registry and to the registrations, then publishes the copy.
  fn update<T>(
    &self,
    change: impl FnOnce(&mut Fonts, &mut FontRegistrations) -> Result<T, FontError>,
  ) -> Result<T, FontError> {
    let mut fonts = Fonts::clone(&self.current());
    let changed = change(&mut fonts, &mut self.registrations.borrow_mut())?;

    *self.fonts.borrow_mut() = Rc::new(fonts);

    Ok(changed)
  }

  /// Unregisters what the font files the cache evicted held.
  fn release_evicted(&self) {
    let evicted = self.cache.fonts().take_evicted();

    if evicted.is_empty() {
      return;
    }

    let _ = self.update(|fonts, registrations| {
      registrations.release(fonts, &evicted);

      Ok(())
    });
  }

  /// The families a registration under `key` added, while the cache holds its file. Leases the file
  /// to `lease`, or pins it for good when `pin` is set.
  pub fn cached(&self, key: &str, pin: bool, lease: Option<u32>) -> Option<Vec<RegisteredFamily>> {
    self.release_evicted();

    if pin {
      return self.registrations.borrow_mut().pin(&self.cache, key);
    }

    let (families, held) = self.registrations.borrow().lease(&self.cache, key)?;

    self.leases.hold(lease, held);

    Some(families)
  }

  /// Registers `font` under `key`, its file leased to `lease`, kept as `retention` says.
  pub fn register(
    &self,
    font: Font,
    key: Option<&str>,
    retention: Retention,
    lease: Option<u32>,
  ) -> Result<Vec<RegisteredFamily>, FontError> {
    let (families, held) = self.update(|fonts, registrations| {
      let registered = register_font(fonts, font)?;

      Ok(registrations.admit(fonts, &self.cache, key, registered, retention))
    })?;

    self.leases.hold(lease, held);
    self.release_evicted();

    Ok(families)
  }

  /// Removes every face registered under `family`; see [`Fonts::unregister_family`].
  pub fn unregister_family(&self, family: &str) -> bool {
    self
      .update(|fonts, registrations| {
        registrations.forget_family(&self.cache, family);

        Ok(fonts.unregister_family(family))
      })
      .unwrap_or_default()
  }

  /// Loads the `@font-face` file `url` names, as a needs pass reported it, leased to `lease`.
  pub fn cache_font_file(
    &self,
    url: &str,
    data: Vec<u8>,
    lease: Option<u32>,
  ) -> Result<(), FontError> {
    let (source, bytes) = FontSource::from(data).into_decoded()?;
    let held = self.update(|fonts, registrations| {
      registrations.admit_file(fonts, &self.cache, url, source, bytes as u64)
    })?;

    self.leases.hold(lease, held);
    self.release_evicted();

    Ok(())
  }

  /// Whether the cache still holds the `@font-face` file `url`, leasing it to `lease` when it does.
  pub fn cached_font_file(&self, url: &str, lease: u32) -> bool {
    self.release_evicted();

    let Some(held) = self.registrations.borrow().lease_file(&self.cache, url) else {
      return false;
    };

    self.leases.hold(Some(lease), held);

    true
  }

  /// Opens a set of leases a render holds until [`Self::release_fonts`].
  pub fn lease_fonts(&self) -> u32 {
    self.leases.open()
  }

  /// Releases the leases of the set `lease` names, evicting what no longer fits.
  pub fn release_fonts(&self, lease: u32) {
    self.leases.close(lease);
    self.release_evicted();
  }

  /// Forgets the fonts registered under `keys` that no render leases; see
  /// [`FontRegistrations::forget_unleased`].
  pub fn drop_fonts(&self, keys: &[String]) -> Vec<String> {
    self.release_evicted();
    self
      .update(|fonts, registrations| Ok(registrations.forget_unleased(fonts, &self.cache, keys)))
      .unwrap_or_default()
  }

  /// Gives the faces registered under `keys` precedence in their families, in that order.
  pub fn order_fonts(&self, keys: &[String]) {
    let mut fonts = Fonts::clone(&self.current());

    if self.registrations.borrow().prefer(&mut fonts, keys) {
      *self.fonts.borrow_mut() = Rc::new(fonts);
    }
  }

  /// Goes back to the embedded last-resort fonts alone.
  pub fn reset(&self) -> Result<(), FontError> {
    *self.fonts.borrow_mut() = Rc::new(default_fonts()?);
    self.registrations.borrow_mut().clear(&self.cache);

    Ok(())
  }
}

/// `error` as the JavaScript error a wasm binding throws.
#[cfg(feature = "wasm")]
fn js_error(error: impl Display) -> js_sys::Error {
  js_sys::Error::new(&error.to_string())
}

/// A render's default language, parsed from its BCP-47 tag.
pub fn parse_lang(tag: Option<&str>) -> Result<Option<Lang>, CoreError> {
  tag.map(Lang::parse).transpose()
}

/// The timeline position in milliseconds, with negative times clamped to zero.
pub fn time_ms(time_ms: Option<i64>) -> u64 {
  time_ms.unwrap_or_default().max(0) as u64
}

/// The device pixel ratio, defaulting to [`DEFAULT_DEVICE_PIXEL_RATIO`].
pub fn device_pixel_ratio(ratio: Option<f32>) -> f32 {
  ratio.unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO)
}

/// The CSS for a render, taking the deprecated `stylesheets` alias when `css`
/// is absent.
pub fn css_or_stylesheets(
  css: Option<Vec<CssSource>>,
  stylesheets: Option<Vec<String>>,
) -> Option<Vec<CssSource>> {
  css.or_else(|| stylesheets.map(|sheets| sheets.into_iter().map(CssSource::Text).collect()))
}

/// The CSS for a render followed by `document`, the CSS of an HTML source's `<style>` elements.
pub fn append_css(css: Option<Vec<CssSource>>, document: Vec<String>) -> Option<Vec<CssSource>> {
  if document.is_empty() {
    return css;
  }

  let mut css = css.unwrap_or_default();

  css.extend(document.into_iter().map(CssSource::Text));
  Some(css)
}

/// An animation's scene trees, each with how long it shows, and the CSS every scene renders with.
pub struct Scenes {
  /// Each scene's tree and its duration in milliseconds.
  pub nodes: Vec<(Node, u32)>,
  /// The render's CSS followed by the CSS of each HTML scene's `<style>` elements.
  pub css: Option<Vec<CssSource>>,
}

impl Scenes {
  /// `scenes` with each source parsed by `read`, styled by `css` and every scene's `<style>`.
  pub fn read<S, E>(
    scenes: impl IntoIterator<Item = (S, u32)>,
    css: Option<Vec<CssSource>>,
    read: impl Fn(S, &SourceCache) -> Result<ParsedSource, E>,
  ) -> Result<Self, E> {
    let mut document_css = Vec::new();
    let nodes = scenes
      .into_iter()
      .map(|(source, duration_ms)| {
        let (node, css) = read(source, &SourceCache::default())?;

        document_css.extend(css);

        Ok((node, duration_ms))
      })
      .collect::<Result<Vec<_>, E>>()?;

    Ok(Self {
      nodes,
      css: append_css(css, document_css),
    })
  }
}

/// The stylesheet for a render: the loose-parsed sheet list with its keyframes.
/// Parsed sheets are cached by source text in `cache`; per-render keyframes are
/// grafted onto a copy so the cached parse stays pristine.
pub fn stylesheet(
  cache: &Cache,
  css: Option<Vec<CssSource>>,
  keyframes: Vec<KeyframesRule>,
) -> Result<Arc<StyleSheet>, CssSourceError> {
  let sheets = css
    .unwrap_or_default()
    .into_iter()
    .map(CssSource::into_css)
    .collect::<Result<Vec<_>, _>>()?;

  let sheet = cache.parse_stylesheet(sheets);

  if keyframes.is_empty() {
    return Ok(sheet);
  }

  let mut extended = (*sheet).clone();
  extended.extend_keyframes(keyframes);
  Ok(Arc::new(extended))
}

/// What a node tree needs fetched, as a wasm binding hands it to JavaScript.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NeedsOutput {
  images: Vec<String>,
  fonts: Vec<usize>,
  font_files: Vec<String>,
  loaded_font_files: Vec<String>,
}

impl NeedsOutput {
  /// What `needs` reports, with the ids of the `listed` fonts the render loads.
  pub fn new(needs: RenderNeeds, listed: Option<&ListedFonts>) -> Self {
    Self {
      fonts: listed.map_or_else(Vec::new, |listed| listed.to_load(&needs.faces)),
      images: needs.images.into_iter().collect(),
      font_files: needs.font_files.into_iter().collect(),
      loaded_font_files: needs.loaded_font_files.into_iter().collect(),
    }
  }
}
