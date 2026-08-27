//! Platform-agnostic glue shared by the napi and wasm bindings.
//!
//! Both bindings lower raw JS input into a takumi render request the same way —
//! the embedded fallback fonts, a font resource from optional fields, the
//! stylesheet. That lowering lives here so neither binding re-derives it. Each
//! binding keeps only its platform-specific glue (JS type coercion, error
//! mapping, threading).

use std::{
  collections::{BTreeMap, HashMap},
  sync::Arc,
};

use takumi_core::{
  Fonts,
  resources::{
    font::{FontError, FontOverride, FontResource},
    image::ResourceCache,
  },
  style::{FontStyle, KeyframesRule, StyleSheet},
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

/// Builds a font resource from normalized optional fields. Each binding pulls
/// these out of its own input type before calling in.
pub fn build_font_resource<'a>(
  bytes: &'a [u8],
  name: Option<String>,
  weight: Option<f32>,
  style: Option<FontStyle>,
  subset_of: Option<String>,
  subset_rank: Option<u32>,
  generic: Option<String>,
) -> Result<FontResource<'a>, FontError> {
  let resource = FontResource::new(bytes).override_info(FontOverride {
    family_name: name.map(Into::into),
    weight,
    style,
    ..Default::default()
  });

  let resource = match subset_of {
    Some(logical) => resource
      .subset_of(logical)
      .subset_rank(subset_rank.unwrap_or_default()),
    None => resource,
  };

  match generic {
    Some(generic) => Ok(resource.generic_family(generic.parse()?)),
    None => Ok(resource),
  }
}

/// CSS variables as the `:root` rule they stand for. A name without the `--`
/// prefix gains it. A value carrying `{`, `}`, `;` or a comment would escape
/// that rule, and `!important` would outrank declarations it has no business
/// outranking, so those are dropped.
fn css_variables_stylesheet(variables: HashMap<String, String>) -> Option<String> {
  // Sorted, because the sheet cache is keyed by source text.
  let declarations = variables
    .into_iter()
    .map(|(name, value)| {
      let name = match name.starts_with("--") {
        true => name,
        false => format!("--{name}"),
      };

      (name, value)
    })
    .collect::<BTreeMap<_, _>>()
    .into_iter()
    .filter(|(name, value)| {
      !name.contains([':', ';', '{', '}'])
        && !value.contains([';', '{', '}'])
        && !value.contains("/*")
        && !value.to_ascii_lowercase().contains("!important")
    })
    .map(|(name, value)| format!("{name}:{value};"))
    .collect::<String>();

  (!declarations.is_empty()).then(|| format!(":root{{{declarations}}}"))
}

/// The stylesheet for a render: the loose-parsed sheet list with its keyframes.
/// Parsed sheets are cached by source text in `cache`; per-render keyframes are
/// grafted onto a copy so the cached parse stays pristine. CSS variables join
/// the sheet list last, so an equally specific author `:root` loses to them.
pub fn stylesheet(
  cache: &ResourceCache,
  css: Option<Vec<String>>,
  keyframes: Vec<KeyframesRule>,
  css_variables: Option<HashMap<String, String>>,
) -> Arc<StyleSheet> {
  let mut sheets = css.unwrap_or_default();

  if let Some(sheet) = css_variables.and_then(css_variables_stylesheet) {
    sheets.push(sheet);
  }

  let sheet = cache.get_or_parse_stylesheet(sheets);

  if keyframes.is_empty() {
    return sheet;
  }

  let mut extended = (*sheet).clone();
  extended.extend_keyframes(keyframes);
  Arc::new(extended)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn variables(entries: &[(&str, &str)]) -> Option<String> {
    css_variables_stylesheet(
      entries
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect(),
    )
  }

  #[test]
  fn test_variables_become_a_root_rule() {
    assert_eq!(
      variables(&[("--color-brand-500", "#5b21b6")]),
      Some(":root{--color-brand-500:#5b21b6;}".to_owned())
    );
  }

  /// The sheet cache is keyed by source text, so the same variables have to
  /// produce the same string however the map iterates.
  #[test]
  fn test_variables_are_ordered() {
    let entries = [("--b", "2"), ("--a", "1")];

    assert_eq!(variables(&entries), Some(":root{--a:1;--b:2;}".to_owned()));
  }

  #[test]
  fn test_variables_that_would_escape_the_rule_are_dropped() {
    let escaping = [
      ("--a", "red; } body { display: none"),
      ("--b", "red /* comment"),
      ("--c", "red !important"),
      ("--d", "red !IMPORTANT"),
      ("--e{", "red"),
    ];

    for entry in escaping {
      assert_eq!(variables(&[entry]), None, "{entry:?}");
    }
  }

  #[test]
  fn test_bare_names_gain_the_prefix() {
    assert_eq!(
      variables(&[("color-brand-500", "#5b21b6")]),
      Some(":root{--color-brand-500:#5b21b6;}".to_owned())
    );
  }

  #[test]
  fn test_no_variables_means_no_sheet() {
    assert_eq!(variables(&[]), None);
  }
}
