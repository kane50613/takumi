//! Platform-agnostic glue shared by the napi and wasm bindings.
//!
//! Both bindings lower raw JS input into a takumi render request the same way —
//! the embedded fallback fonts, a font resource from optional fields, the
//! stylesheet, and the per-render options. That lowering lives here so neither binding re-derives it. Each
//! binding keeps only its platform-specific glue (JS type coercion, error
//! mapping, threading).

pub mod font_cache;
pub mod input;
#[cfg(feature = "wasm")]
pub mod source;
pub mod source_cache;

#[cfg(feature = "wasm")]
use std::fmt::Display;
use std::{cell::RefCell, rc::Rc, sync::Arc};

use font_cache::FontCache;
use input::{Font, ListedFonts, register_font};
use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use serde_wasm_bindgen::{from_value, to_value};
use source_cache::{ParsedSource, SourceCache};
#[cfg(feature = "wasm")]
use wasm_bindgen::{JsCast, JsValue};

use takumi_core::{
  Error as CoreError, Fonts,
  layout::{needs::RenderNeeds, node::Node},
  resources::{
    font::{FontError, FontOverride, FontResource, FontSource, RegisteredFamily},
    image::ResourceCache,
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
  cache: RefCell<FontCache>,
}

impl FontStore {
  /// A store holding the embedded last-resort fonts, keeping at most `cache_max_bytes` of fonts
  /// that are not pinned.
  pub fn new(cache_max_bytes: Option<u64>) -> Result<Self, FontError> {
    Ok(Self {
      fonts: RefCell::new(Rc::new(default_fonts()?)),
      cache: RefCell::new(FontCache::new(cache_max_bytes)),
    })
  }

  /// The registry for a render that names `families`, which are marked used.
  pub fn for_render(&self, families: Option<&[String]>) -> Rc<Fonts> {
    self.cache.borrow().touch(families.unwrap_or_default());
    self.current()
  }

  /// The registry for a needs pass, with each listed font still loading registered as pending.
  pub fn for_needs(
    &self,
    families: &mut Option<Vec<String>>,
    listed: Option<&ListedFonts>,
  ) -> Rc<Fonts> {
    let Some(listed) = listed else {
      return self.for_render(families.as_deref());
    };

    if families.is_none() {
      families.clone_from(&listed.families);
    }

    Rc::new(listed.fonts(&self.for_render(families.as_deref())))
  }

  /// Registers the font JavaScript describes under `key`. One that is not `pinned` can be released.
  #[cfg(feature = "wasm")]
  pub fn register_value<T: JsCast>(
    &self,
    font: impl Into<JsValue>,
    key: Option<&str>,
    pinned: bool,
  ) -> Result<T, js_sys::Error> {
    let font = Font::from_js(font.into()).map_err(js_error)?;
    let registered = self.register(font, key, pinned).map_err(js_error)?;

    Ok(to_value(&registered).map_err(js_error)?.unchecked_into())
  }

  /// [`Self::cached`] as the binding's JavaScript type `T`.
  #[cfg(feature = "wasm")]
  pub fn cached_value<T: JsCast>(&self, key: &str, pin: bool) -> Result<Option<T>, js_sys::Error> {
    self
      .cached(key, pin)
      .map(|registered| Ok(to_value(&registered).map_err(js_error)?.unchecked_into()))
      .transpose()
  }

  /// A render's `fonts` list, as JavaScript describes it, read against the fonts this store holds.
  #[cfg(feature = "wasm")]
  pub fn listed(&self, listed: Option<JsValue>) -> Result<Option<ListedFonts>, js_sys::Error> {
    listed
      .map(|listed| {
        from_value(listed)
          .map(|listed| ListedFonts::read(&self.cache.borrow(), listed))
          .map_err(js_error)
      })
      .transpose()
  }

  fn current(&self) -> Rc<Fonts> {
    self.fonts.borrow().clone()
  }

  /// The families a registration under `key` added, marked used and pinned when `pin` is set; see
  /// [`FontCache::lookup`] and [`FontCache::pin`].
  pub fn cached(&self, key: &str, pin: bool) -> Option<Vec<RegisteredFamily>> {
    if pin {
      self.cache.borrow_mut().pin(key)
    } else {
      self.cache.borrow().lookup(key)
    }
  }

  /// Registers `font` under `key`; one that is not `pinned` can be released for space.
  pub fn register(
    &self,
    font: Font,
    key: Option<&str>,
    pinned: bool,
  ) -> Result<Vec<RegisteredFamily>, FontError> {
    let mut fonts = Fonts::clone(&self.current());
    let (registered, bytes) = register_font(&mut fonts, font)?;

    self
      .cache
      .borrow_mut()
      .admit(&mut fonts, key, &registered, bytes, pinned)?;
    *self.fonts.borrow_mut() = Rc::new(fonts);

    Ok(registered)
  }

  /// Removes every face registered under `family`; see [`Fonts::unregister_family`].
  pub fn unregister_family(&self, family: &str) -> bool {
    let mut fonts = Fonts::clone(&self.current());
    let removed = fonts.unregister_family(family);

    self.cache.borrow_mut().forget(family);

    if removed {
      *self.fonts.borrow_mut() = Rc::new(fonts);
    }

    removed
  }

  /// Goes back to the embedded last-resort fonts alone.
  pub fn reset(&self) -> Result<(), FontError> {
    *self.fonts.borrow_mut() = Rc::new(default_fonts()?);
    self.cache.borrow_mut().clear();

    Ok(())
  }
}

/// `error` as the JavaScript error a wasm binding throws.
#[cfg(feature = "wasm")]
fn js_error(error: impl Display) -> js_sys::Error {
  js_sys::Error::new(&error.to_string())
}

/// Options for a renderer whose only option is its font cache budget.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FontCacheOptions {
  /// Bytes of fonts from renders' `fonts` lists to keep registered.
  pub font_cache_max_bytes: Option<u64>,
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
  cache: &ResourceCache,
  css: Option<Vec<CssSource>>,
  keyframes: Vec<KeyframesRule>,
) -> Result<Arc<StyleSheet>, CssSourceError> {
  let sheets = css
    .unwrap_or_default()
    .into_iter()
    .map(CssSource::into_css)
    .collect::<Result<Vec<_>, _>>()?;

  let sheet = cache.get_or_parse_stylesheet(sheets);

  if keyframes.is_empty() {
    return Ok(sheet);
  }

  let mut extended = (*sheet).clone();
  extended.extend_keyframes(keyframes);
  Ok(Arc::new(extended))
}

/// What a node tree needs fetched, as a wasm binding hands it to JavaScript.
#[derive(Serialize)]
pub struct NeedsOutput {
  images: Vec<String>,
  fonts: Vec<usize>,
}

impl NeedsOutput {
  /// What `needs` reports, with the ids of the `listed` fonts the render loads.
  pub fn new(needs: RenderNeeds, listed: Option<&ListedFonts>) -> Self {
    Self {
      fonts: listed.map_or_else(Vec::new, |listed| listed.to_load(&needs.faces)),
      images: needs.images.into_iter().collect(),
    }
  }
}
