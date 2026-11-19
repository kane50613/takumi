//! Platform-agnostic glue shared by the napi and wasm bindings.
//!
//! Both bindings lower raw JS input into a takumi render request the same way —
//! the embedded fallback fonts, a font resource from optional fields, the
//! stylesheet, and the per-render options. That lowering lives here so neither binding re-derives it. Each
//! binding keeps only its platform-specific glue (JS type coercion, error
//! mapping, threading).

#![deny(
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic,
  clippy::unreachable,
  clippy::todo,
  clippy::unimplemented
)]
#![cfg_attr(
  test,
  allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
  )
)]
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
pub mod warning;

#[cfg(feature = "wasm")]
use std::fmt::Display;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use input::{Font, ListedFont, ListedFonts, RegisteredFont, load_font};
use registrations::{FontRegistrations, RenderLeases, Retention};
#[cfg(feature = "wasm")]
use renderer::CacheStats;
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

/// The system fonts every renderer starts with, by family name.
const EMBEDDED_FONTS: &[(&[u8], &str)] = &[(
  include_bytes!("../../assets/fonts/geist/geist-latin-wght-300-800.ttf"),
  "Geist",
)];

/// The default font set, with the embedded fonts as its system fonts.
pub fn default_fonts() -> Result<Fonts, FontError> {
  Fonts::with_system_fonts(EMBEDDED_FONTS.iter().map(|(bytes, name)| {
    FontResource::new(FontSource::from_static(bytes)).override_info(FontOverride {
      family_name: Some((*name).into()),
      ..Default::default()
    })
  }))
}

/// A renderer's registered fonts, the registrations that added them, and the leases its renders
/// hold, for every binding. A change publishes a new registry, so a render keeps the fonts it
/// started with.
pub struct FontStore {
  /// Every call comes from the binding's JS thread, so the lock never waits. It makes the store
  /// `Sync` for the napi tasks that carry it, and render workers never take it.
  book: Mutex<FontBook>,
  cache: Cache,
}

struct FontBook {
  fonts: Arc<Fonts>,
  registrations: FontRegistrations,
  /// The keys the render that ordered its fonts last listed, whose fonts every registry handed to a
  /// render adds again last, a later key over an earlier one.
  listed: Vec<String>,
  leases: RenderLeases,
}

impl FontStore {
  /// A store holding the embedded system fonts, whose font files `cache` holds.
  pub fn new(cache: Cache) -> Result<Self, FontError> {
    Ok(Self {
      book: Mutex::new(FontBook {
        fonts: Arc::new(default_fonts()?),
        registrations: FontRegistrations::default(),
        listed: Vec::new(),
        leases: RenderLeases::default(),
      }),
      cache,
    })
  }

  /// The registry as it is now.
  pub fn current(&self) -> Arc<Fonts> {
    Arc::clone(&self.book().fonts)
  }

  /// The registry for a render, with the `@font-face` files its document `failed` to download.
  pub fn for_render(&self, failed: Option<&[String]>) -> Arc<Fonts> {
    self.release_evicted();

    let failed = failed.unwrap_or_default();
    let fonts = {
      let book = self.book();

      book.registrations.listing(&book.fonts, &book.listed)
    };

    if failed.is_empty() {
      fonts
    } else {
      Arc::new(fonts.with_failed_font_files(failed))
    }
  }

  /// The registry for a needs pass, with each listed font still loading registered as pending.
  pub fn for_needs(
    &self,
    families: &mut Option<Vec<String>>,
    listed: Option<&ListedFonts>,
    failed: Option<&[String]>,
  ) -> Arc<Fonts> {
    let Some(listed) = listed else {
      return self.for_render(failed);
    };

    if families.is_none() {
      families.clone_from(&listed.families);
    }

    Arc::new(listed.fonts(&self.for_render(failed)))
  }

  /// A render's `fonts` list read against the fonts this store holds.
  pub fn read_listed(&self, listed: Vec<ListedFont>) -> ListedFonts {
    self.release_evicted();

    ListedFonts::read(&self.book().registrations, listed)
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
    listed
      .map(|listed| {
        from_value(listed)
          .map(|listed| self.read_listed(listed))
          .map_err(js_error)
      })
      .transpose()
  }

  /// What the cache holds, as the binding's JavaScript type `T`.
  #[cfg(feature = "wasm")]
  pub fn cache_stats_value<T: JsCast>(&self) -> Result<T, js_sys::Error> {
    let stats = CacheStats::new(&self.cache)
      .serialize(&Serializer::new().serialize_missing_as_null(true))
      .map_err(js_error)?;

    Ok(stats.unchecked_into())
  }

  fn book(&self) -> MutexGuard<'_, FontBook> {
    self.book.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// Applies `change` to copies of the registry and the registrations, then publishes both, so a
  /// change that fails, or panics in a napi build that unwinds, leaves the store as it was.
  fn update<T>(
    &self,
    change: impl FnOnce(&mut Fonts, &mut FontRegistrations) -> Result<T, FontError>,
  ) -> Result<T, FontError> {
    let mut book = self.book();
    let mut fonts = Fonts::clone(&book.fonts);
    let mut registrations = book.registrations.clone();
    let changed = change(&mut fonts, &mut registrations)?;

    book.fonts = Arc::new(fonts);
    book.registrations = registrations;

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
      return self
        .update(|_, registrations| Ok(registrations.pin(&self.cache, key)))
        .unwrap_or_default();
    }

    let mut book = self.book();
    let (families, held) = book.registrations.lease(&self.cache, key)?;

    book.leases.hold(lease, held);

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
    self.admit(key, retention, lease, |fonts| load_font(fonts, font))
  }

  /// Registers `resource`, which [`FontResource::into_resolved`] gave, under `key`, its file leased
  /// to `lease`, kept as `retention` says.
  pub fn register_resolved(
    &self,
    resource: FontResource<'_>,
    key: Option<&str>,
    retention: Retention,
    lease: Option<u32>,
  ) -> Result<Vec<RegisteredFamily>, FontError> {
    self.admit(key, retention, lease, |fonts| {
      RegisteredFont::load(fonts, resource)
    })
  }

  /// Records the font `register` adds under `key`, its file leased to `lease`, then unregisters the
  /// fonts the cache evicted to make room.
  fn admit(
    &self,
    key: Option<&str>,
    retention: Retention,
    lease: Option<u32>,
    register: impl FnOnce(&mut Fonts) -> Result<RegisteredFont, FontError>,
  ) -> Result<Vec<RegisteredFamily>, FontError> {
    let (families, held) = self.update(|fonts, registrations| {
      let registered = register(fonts)?;

      Ok(registrations.admit(fonts, &self.cache, key, registered, retention))
    })?;

    self.book().leases.hold(lease, held);
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

    self.load_font_file(url, source, bytes as u64, lease)
  }

  /// Loads the `@font-face` file `url` names, `bytes` of decoded font data, leased to `lease`.
  pub fn load_font_file(
    &self,
    url: &str,
    source: FontSource<'static>,
    bytes: u64,
    lease: Option<u32>,
  ) -> Result<(), FontError> {
    let held = self.update(|fonts, registrations| {
      registrations.admit_file(fonts, &self.cache, url, source, bytes)
    })?;

    self.book().leases.hold(lease, held);
    self.release_evicted();

    Ok(())
  }

  /// Whether the cache still holds the `@font-face` file `url`, leasing it to `lease` when it does.
  pub fn cached_font_file(&self, url: &str, lease: u32) -> bool {
    self.release_evicted();

    let mut book = self.book();
    let Some(held) = book.registrations.lease_file(&self.cache, url) else {
      return false;
    };

    book.leases.hold(Some(lease), held);

    true
  }

  /// Opens a set of leases a render holds until [`Self::release_fonts`].
  pub fn lease_fonts(&self) -> u32 {
    self.book().leases.open()
  }

  /// Releases the leases of the set `lease` names, evicting what no longer fits.
  pub fn release_fonts(&self, lease: u32) {
    let released = self.book().leases.close(lease);

    // Released after the lock, since releasing can evict.
    drop(released);
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

  /// Gives the faces registered under `keys` precedence in their families, a later key over an
  /// earlier one, in every registry handed to a render from now on.
  pub fn order_fonts(&self, keys: &[String]) {
    self.book().listed = keys.to_vec();
  }

  /// Goes back to the embedded system fonts alone.
  pub fn reset(&self) -> Result<(), FontError> {
    self.update(|fonts, registrations| {
      *fonts = default_fonts()?;
      registrations.clear(&self.cache);

      Ok(())
    })
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
