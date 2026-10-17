//! Platform-agnostic glue shared by the napi and wasm bindings.
//!
//! Both bindings lower raw JS input into a takumi render request the same way —
//! the embedded fallback fonts, a font resource from optional fields, the
//! stylesheet, and the per-render options. That lowering lives here so neither binding re-derives it. Each
//! binding keeps only its platform-specific glue (JS type coercion, error
//! mapping, threading).

pub mod input;

use std::{cell::RefCell, rc::Rc, sync::Arc};

use takumi_core::{
  Error as CoreError, Fonts,
  resources::{
    font::{FontError, FontOverride, FontResource},
    image::ResourceCache,
  },
  style::{CssSource, CssSourceError, KeyframesRule, Lang, StyleSheet},
  viewport::DEFAULT_DEVICE_PIXEL_RATIO,
};

/// Last-resort only: no generic family claim, so `sans-serif` and friends
/// resolve to caller-registered fonts via the fallback bucket instead of this
/// face.
const EMBEDDED_FONTS: &[(&[u8], &str)] = &[(
  include_bytes!("../../assets/fonts/geist/geist-latin-wght-300-800.woff2"),
  "Geist",
)];

/// The default font set holding the embedded last-resort fonts.
pub fn default_fonts() -> Result<Fonts, FontError> {
  let mut fonts = Fonts::default();

  for (bytes, name) in EMBEDDED_FONTS {
    let resource = FontResource::new(*bytes)
      .override_info(FontOverride {
        family_name: Some((*name).into()),
        ..Default::default()
      })
      .last_resort();

    drop(fonts.register(resource)?);
  }

  Ok(fonts)
}

/// Registered fonts for the single-threaded wasm bindings, copy-on-write like the napi
/// bindings' `ArcSwap`: a call takes the current registry and lets go of the store at once, and
/// a change publishes a new registry. Nothing is held across a call, so one that traps (release
/// builds abort on panic) leaves the store usable.
pub struct FontStore(RefCell<Rc<Fonts>>);

impl FontStore {
  /// A store holding the embedded last-resort fonts.
  pub fn new() -> Result<Self, FontError> {
    default_fonts().map(|fonts| Self(RefCell::new(Rc::new(fonts))))
  }

  /// The registry as it is now.
  pub fn current(&self) -> Rc<Fonts> {
    self.0.borrow().clone()
  }

  /// Applies `change` to a copy of the registry, publishing the copy only when it succeeds.
  pub fn update<T, E>(&self, change: impl FnOnce(&mut Fonts) -> Result<T, E>) -> Result<T, E> {
    let mut fonts = Fonts::clone(&self.current());
    let changed = change(&mut fonts)?;

    *self.0.borrow_mut() = Rc::new(fonts);

    Ok(changed)
  }
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
