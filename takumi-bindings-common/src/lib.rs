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

use std::{cell::RefCell, rc::Rc, sync::Arc};

use font_cache::FontCache;
use input::{Font, register_font};
use serde::{
  Deserialize, Serialize,
  de::{IntoDeserializer, value::Error as DeError},
};

use takumi_core::{
  Error as CoreError, Fonts,
  layout::{needs::RenderNeeds, node::EmojiProvider},
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
    self.cache.borrow_mut().touch(families.unwrap_or_default());
    self.current()
  }

  fn current(&self) -> Rc<Fonts> {
    self.fonts.borrow().clone()
  }

  /// The families a registration under `key` added, marked used; see [`FontCache::lookup`].
  pub fn cached(&self, key: &str, pin: bool) -> Option<Vec<RegisteredFamily>> {
    self.cache.borrow_mut().lookup(key, pin)
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

/// The provider a render's `emoji` option names, `None` for `from-font` or no option.
pub fn parse_emoji(name: Option<&str>) -> Result<Option<EmojiProvider>, DeError> {
  match name {
    None | Some("from-font") => Ok(None),
    Some(name) => EmojiProvider::deserialize(name.into_deserializer()).map(Some),
  }
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
  characters: String,
  faces: Vec<usize>,
}

impl From<RenderNeeds> for NeedsOutput {
  fn from(needs: RenderNeeds) -> Self {
    Self {
      images: needs.images.into_iter().collect(),
      characters: needs.characters.into_iter().collect(),
      faces: needs.faces.into_iter().collect(),
    }
  }
}
