//! `@font-face` rules as Blink's [`AtRuleDescriptorParser`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/parser/at_rule_descriptor_parser.cc)
//! reads them, and the face [`FontFace::Create`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/font_face.cc)
//! makes of one.
//!
//! Approximate: `font-feature-settings` is ignored, and a `local()` source never matches.
//! `font-display` has no effect, as a render draws with the files that have loaded by then.

use std::sync::Arc;

use cssparser::{Parser, Token, match_ignore_ascii_case};
use harfrust::font::Variation;

use crate::{
  fonts::font_face::FontFaceDescriptors,
  resources::unicode_range::{UnicodeRange, UnicodeRangeSet},
  style::{
    FontStretch, FontStyle, FromCss, GenericFamilyType, MediaQueryList, RawDeclaration,
    RawDeclarationParser,
    properties::{FontVariationSettings, oblique_angle},
    selector::LayerPath,
  },
};

/// An `@font-face` rule with its `font-family` and `src`, the descriptors `FontFace::Create`
/// requires.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FontFaceRule {
  /// The descriptors, `family` always set.
  pub(crate) descriptors: FontFaceDescriptors,
  /// The `url()` sources Blink's `InitCSSFontFace` keeps, in order.
  pub(crate) sources: Vec<Arc<str>>,
  /// Media queries gating this rule.
  pub(crate) media_queries: Vec<MediaQueryList>,
  /// Cascade layer this rule belongs to.
  pub(crate) layer: Option<LayerPath>,
  /// Resolved ordinal of the rule's layer.
  pub(crate) layer_order: Option<usize>,
}

/// A `src` entry as `ConsumeFontFaceSrc` keeps it.
enum FontFaceSrc {
  /// A `url()`, and whether it names a `format()`.
  Url {
    url: Arc<str>,
    has_format: bool,
  },
  Local,
}

impl FontFaceSrc {
  /// Blink's `CSSFontFaceSrcValue::IsSupportedFormat`: the parser already dropped an unsupported
  /// `format()`, and without one only an `.eot` URL is left out.
  fn supported_url(self) -> Option<Arc<str>> {
    match self {
      Self::Url { url, has_format } => {
        (has_format || url.starts_with("data:") || !url.to_ascii_lowercase().ends_with(".eot"))
          .then_some(url)
      }
      Self::Local => None,
    }
  }
}

impl FontFaceRule {
  /// Blink's `FontFace::Create` from the rule's descriptors, `None` without a valid `font-family`
  /// and `src`.
  pub(crate) fn parse(input: &mut Parser) -> Option<Self> {
    let mut parser = RawDeclarationParser;
    let mut family = None;
    let mut sources = None;
    // Blink's `FontFace` leaves a descriptor the rule omits at `normal`, unlike an `auto` one.
    let mut descriptors = FontFaceDescriptors {
      weight: Some((400.0, 400.0)),
      width: Some((100.0, 100.0)),
      slope: Some((0.0, 0.0)),
      ..FontFaceDescriptors::default()
    };

    for descriptor in cssparser::RuleBodyParser::new(input, &mut parser) {
      let Ok(RawDeclaration {
        name,
        value,
        important: false,
      }) = descriptor
      else {
        continue;
      };
      let mut value = Parser::new(value);
      let value = &mut value;

      match_ignore_ascii_case! { &name,
        "font-family" => if let Some(parsed) = descriptor_value(value, family_name) {
          family = Some(parsed);
        },
        "src" => if let Some(parsed) = descriptor_value(value, src_list) {
          sources = Some(parsed);
        },
        "font-weight" => if let Some(parsed) = descriptor_value(value, weight_range) {
          descriptors.weight = parsed;
        },
        "font-style" => if let Some(parsed) = descriptor_value(value, slope_range) {
          descriptors.slope = parsed;
        },
        "font-stretch" => if let Some(parsed) = descriptor_value(value, width_range) {
          descriptors.width = parsed;
        },
        "unicode-range" => if let Some(parsed) = descriptor_value(value, unicode_ranges) {
          descriptors.unicode_range = parsed;
        },
        "font-variation-settings" => if let Some(parsed) = descriptor_value(value, variation_settings) {
          descriptors.variation_settings = parsed;
        },
        "size-adjust" => if let Some(parsed) = descriptor_value(value, non_negative_fraction) {
          descriptors.size_adjust = Some(parsed);
        },
        "ascent-override" => if let Some(parsed) = descriptor_value(value, metric_override) {
          descriptors.metrics_override.ascent = parsed;
        },
        "descent-override" => if let Some(parsed) = descriptor_value(value, metric_override) {
          descriptors.metrics_override.descent = parsed;
        },
        "line-gap-override" => if let Some(parsed) = descriptor_value(value, metric_override) {
          descriptors.metrics_override.line_gap = parsed;
        },
        _ => {},
      }
    }

    let family = family?;
    let sources = sources?
      .into_iter()
      .filter_map(FontFaceSrc::supported_url)
      .collect();

    descriptors.family = Some(family);

    Some(Self {
      descriptors,
      sources,
      media_queries: Vec::new(),
      layer: None,
      layer_order: None,
    })
  }
}

/// A descriptor's value when `parse` reads all of it, as `ParseFontFaceDescriptor` requires.
fn descriptor_value<'i, T>(
  input: &mut Parser<'i>,
  parse: impl FnOnce(&mut Parser<'i>) -> Option<T>,
) -> Option<T> {
  let parsed = parse(input)?;

  input.expect_exhausted().ok()?;

  Some(parsed)
}

/// `font-family`: a `<family-name>`, never a generic family or a CSS-wide keyword.
fn family_name(input: &mut Parser) -> Option<Arc<str>> {
  if let Ok(name) = input.try_parse(|input| input.expect_string().map(ToString::to_string)) {
    return Some(name.into());
  }

  let mut name = String::new();

  while let Ok(ident) = input.try_parse(Parser::expect_ident_cloned) {
    if name.is_empty() && GenericFamilyType::parse(&ident).is_some() {
      return None;
    }

    if is_css_wide_keyword(&ident) {
      return None;
    }

    if !name.is_empty() {
      name.push(' ');
    }

    name.push_str(&ident);
  }

  (!name.is_empty()).then(|| name.into())
}

fn is_css_wide_keyword(ident: &str) -> bool {
  match_ignore_ascii_case! { ident,
    "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default" => true,
    _ => false,
  }
}

/// Blink's `ConsumeFontFaceSrc`: the entries that parse, an invalid one skipped to the next comma.
fn src_list(input: &mut Parser) -> Option<Vec<FontFaceSrc>> {
  let mut sources = Vec::new();

  loop {
    let start = input.state();

    match input.try_parse(src_entry) {
      Ok(source) => sources.push(source),
      Err(()) => {
        input.reset(&start);

        while !input.is_exhausted() {
          if input.try_parse(Parser::expect_comma).is_ok() {
            break;
          }

          let _ = input.next();
        }

        if input.is_exhausted() {
          break;
        }

        continue;
      }
    }

    if input.try_parse(Parser::expect_comma).is_err() {
      break;
    }
  }

  (!sources.is_empty()).then_some(sources)
}

/// One `src` entry, ending at a comma or the end of the value.
fn src_entry(input: &mut Parser) -> Result<FontFaceSrc, ()> {
  if input
    .try_parse(|input| input.expect_function_matching("local"))
    .is_ok()
  {
    input
      .parse_nested_block(|block| {
        let valid = block
          .try_parse(|block| block.expect_string().map(|_| ()))
          .is_ok()
          || family_name(block).is_some();

        if valid {
          Ok(())
        } else {
          Err(block.new_error_for_next_token::<()>())
        }
      })
      .map_err(|_| ())?;

    return src_end(input).map(|()| FontFaceSrc::Local);
  }

  let url: Arc<str> = input.expect_url().map_err(|_| ())?.as_ref().into();
  let has_format = input
    .try_parse(|input| input.expect_function_matching("format"))
    .is_ok();

  if has_format {
    input
      .parse_nested_block(|block| {
        if is_supported_format(block) {
          Ok(())
        } else {
          Err(block.new_error_for_next_token::<()>())
        }
      })
      .map_err(|_| ())?;
  }

  if input
    .try_parse(|input| input.expect_function_matching("tech"))
    .is_ok()
  {
    input
      .parse_nested_block(|block| {
        block.parse_comma_separated(|block| {
          let tech = block.expect_ident_cloned()?;

          if is_supported_tech(&tech) {
            Ok(())
          } else {
            Err(block.new_error_for_next_token::<()>())
          }
        })
      })
      .map_err(|_| ())?;
  }

  src_end(input).map(|()| FontFaceSrc::Url { url, has_format })
}

/// Succeeds at a comma or the end of the value, without consuming the comma.
fn src_end(input: &mut Parser) -> Result<(), ()> {
  let state = input.state();
  let at_end = input.is_exhausted() || matches!(input.next(), Ok(Token::Comma));

  input.reset(&state);

  if at_end { Ok(()) } else { Err(()) }
}

/// Whether the `format()` argument is one Blink's `IsSupportedFontFormat` accepts: a keyword, or a
/// string that may also name a `-variations` format.
fn is_supported_format(input: &mut Parser) -> bool {
  let (format, is_string) = match input.next() {
    Ok(Token::Ident(format)) => (format.clone(), false),
    Ok(Token::QuotedString(format)) => (format.clone(), true),
    _ => return false,
  };

  input.is_exhausted()
    && match_ignore_ascii_case! { &format,
      "collection" | "opentype" | "truetype" | "woff" | "woff2" => true,
      "woff-variations" | "truetype-variations" | "opentype-variations" | "woff2-variations" => is_string,
      _ => false,
    }
}

/// Blink's `IsSupportedKeywordTech` with its experimental technologies off.
fn is_supported_tech(tech: &str) -> bool {
  match_ignore_ascii_case! { tech,
    "features-opentype" | "features-aat" | "color-colrv0" | "color-colrv1" | "color-sbix"
    | "color-cbdt" | "variations" | "palettes" => true,
    _ => false,
  }
}

/// `auto`, or a range whose ends swap to keep it increasing, as CSS Fonts 4 asks.
/// <https://drafts.csswg.org/css-fonts/#font-prop-desc>
fn range<'i>(
  input: &mut Parser<'i>,
  mut value: impl FnMut(&mut Parser<'i>) -> Option<f32>,
) -> Option<Option<(f32, f32)>> {
  if input
    .try_parse(|input| input.expect_ident_matching("auto"))
    .is_ok()
  {
    return Some(None);
  }

  let start = value(input)?;
  let end = if input.is_exhausted() {
    start
  } else {
    value(input)?
  };

  Some(Some((start.min(end), start.max(end))))
}

/// `font-weight`: `normal`, `bold`, or a number from 1 to 1000.
fn weight_range(input: &mut Parser) -> Option<Option<(f32, f32)>> {
  range(input, |input| {
    input
      .try_parse(|input| {
        let ident = input.expect_ident_cloned()?;

        match_ignore_ascii_case! { &ident,
          "normal" => Ok(400.0),
          "bold" => Ok(700.0),
          _ => Err(input.new_error_for_next_token::<()>()),
        }
      })
      .or_else(|_| input.expect_number().map_err(|_| ()))
      .ok()
      .filter(|weight| (1.0..=1000.0).contains(weight))
  })
}

/// `font-style`: `normal`, `italic`, or `oblique` with up to two angles.
fn slope_range(input: &mut Parser) -> Option<Option<(f32, f32)>> {
  if input
    .try_parse(|input| input.expect_ident_matching("auto"))
    .is_ok()
  {
    return Some(None);
  }

  if input
    .try_parse(|input| input.expect_ident_matching("oblique"))
    .is_ok()
  {
    let Ok(start) = input.try_parse(oblique_angle) else {
      let slope = FontStyle::italic().slope();

      return Some(Some((slope, slope)));
    };
    let end = input.try_parse(oblique_angle).unwrap_or(start);

    return Some(Some((start.min(end), start.max(end))));
  }

  let slope = FontStyle::from_css(input).ok()?.slope();

  Some(Some((slope, slope)))
}

/// `font-stretch`: a keyword, or one or two non-negative percentages.
fn width_range(input: &mut Parser) -> Option<Option<(f32, f32)>> {
  range(input, |input| {
    FontStretch::from_css(input)
      .ok()
      .map(|stretch| stretch.percentage() * 100.0)
  })
}

/// Blink's `ConsumeFontFaceUnicodeRange`.
fn unicode_ranges(input: &mut Parser) -> Option<UnicodeRangeSet> {
  input
    .parse_comma_separated(|input| {
      let range = cssparser::UnicodeRange::parse(input)?;

      if range.start > range.end || range.end > 0x10_FFFF {
        return Err(input.new_error_for_next_token::<()>());
      }

      Ok(UnicodeRange::new(range.start, range.end))
    })
    .ok()
    .map(UnicodeRangeSet::from_iter)
}

/// A non-negative percentage, as a fraction.
fn non_negative_fraction(input: &mut Parser) -> Option<f32> {
  input
    .expect_percentage()
    .ok()
    .filter(|fraction| *fraction >= 0.0)
}

/// Blink's `ConsumeFontMetricOverride`: `normal`, or a non-negative percentage as a fraction.
fn metric_override(input: &mut Parser) -> Option<Option<f32>> {
  if input
    .try_parse(|input| input.expect_ident_matching("normal"))
    .is_ok()
  {
    return Some(None);
  }

  non_negative_fraction(input).map(Some)
}

/// `font-variation-settings`, its tags as harfrust reads them.
fn variation_settings(input: &mut Parser) -> Option<Vec<Variation>> {
  let settings = FontVariationSettings::from_css(input).ok()?;

  Some(
    settings
      .iter()
      .map(|setting| Variation {
        tag: setting.tag.into_harfrust(),
        value: setting.value,
      })
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::StyleSheet;

  fn rules(css: &str) -> Vec<FontFaceRule> {
    StyleSheet::parse(css).unwrap().font_faces().to_vec()
  }

  fn rule(descriptors: &str) -> FontFaceRule {
    let mut rules = rules(&format!(
      "@font-face {{ font-family: Face; {descriptors} }}"
    ));

    assert_eq!(rules.len(), 1, "{descriptors}");
    rules.remove(0)
  }

  fn sources(src: &str) -> Vec<String> {
    rule(&format!("src: {src};"))
      .sources
      .iter()
      .map(ToString::to_string)
      .collect()
  }

  #[test]
  fn a_rule_needs_a_family_and_a_src() {
    assert!(rules("@font-face { src: url(a.woff2); }").is_empty());
    assert!(rules("@font-face { font-family: Face; }").is_empty());
    assert!(rules("@font-face { font-family: serif; src: url(a.woff2); }").is_empty());
    assert!(rules("@font-face { font-family: inherit; src: url(a.woff2); }").is_empty());
    assert_eq!(
      rule("src: url(a.woff2);").descriptors.family.as_deref(),
      Some("Face")
    );
  }

  #[test]
  fn src_keeps_the_urls_a_supported_format_allows() {
    assert_eq!(
      sources(r#"url(a.woff2) format("woff2"), url(b.woff) format(woff)"#),
      ["a.woff2", "b.woff"]
    );
    assert_eq!(sources(r#"url(a.svg) format(svg), url(b.ttf)"#), ["b.ttf"]);
    assert_eq!(
      sources(r#"url(a.eot), url(b.otf) format("opentype")"#),
      ["b.otf"]
    );
    assert_eq!(
      sources(r#"local(Face), url(a.ttf) tech(variations)"#),
      ["a.ttf"]
    );
    assert_eq!(
      sources(r#"url(a.ttf) tech(color-svg), url(b.ttf)"#),
      ["b.ttf"]
    );
    assert_eq!(sources(r#"url(a.ttf) junk, url(b.ttf)"#), ["b.ttf"]);
  }

  #[test]
  fn descriptors_default_to_normal_and_ranges_increase() {
    let defaults = rule("src: url(a.woff2);").descriptors;

    assert_eq!(defaults.weight, Some((400.0, 400.0)));
    assert_eq!(defaults.width, Some((100.0, 100.0)));
    assert_eq!(defaults.slope, Some((0.0, 0.0)));

    let ranged = rule(
      "src: url(a.woff2); font-weight: 700 300; font-stretch: 125% 75%;
       font-style: oblique 20deg 10deg;",
    )
    .descriptors;

    assert_eq!(ranged.weight, Some((300.0, 700.0)));
    assert_eq!(ranged.width, Some((75.0, 125.0)));
    assert_eq!(ranged.slope, Some((10.0, 20.0)));
    assert_eq!(
      rule("src: url(a.woff2); font-weight: auto;")
        .descriptors
        .weight,
      None
    );
    assert_eq!(
      rule("src: url(a.woff2); font-style: italic;")
        .descriptors
        .slope,
      Some((14.0, 14.0))
    );
  }

  #[test]
  fn an_invalid_descriptor_value_is_dropped() {
    let descriptors = rule(
      "src: url(a.woff2); font-weight: 0; unicode-range: U+FF-0; font-stretch: 100% !important;",
    )
    .descriptors;

    assert_eq!(descriptors.weight, Some((400.0, 400.0)));
    assert!(descriptors.unicode_range.is_entire_range());
    assert_eq!(descriptors.width, Some((100.0, 100.0)));
  }

  #[test]
  fn size_adjust_and_metric_overrides_read_as_fractions() {
    let descriptors = rule(
      "src: url(a.woff2); size-adjust: 150%; ascent-override: 90%; descent-override: normal;
       line-gap-override: -10%;",
    )
    .descriptors;

    assert_eq!(descriptors.size_adjust, Some(1.5));
    assert_eq!(descriptors.metrics_override.ascent, Some(0.9));
    assert_eq!(descriptors.metrics_override.descent, None);
    assert_eq!(descriptors.metrics_override.line_gap, None);
  }

  #[test]
  fn unicode_range_reads_each_range() {
    let range = rule("src: url(a.woff2); unicode-range: U+0-7F, U+4??;")
      .descriptors
      .unicode_range;

    assert!(range.contains(0x41));
    assert!(range.contains(0x4FF));
    assert!(!range.contains(0x500));
  }

  #[test]
  fn a_rule_in_a_style_rule_fails_the_sheet() {
    assert!(
      StyleSheet::parse(".a { @font-face { font-family: Face; src: url(a.woff2); } }").is_err()
    );
  }
}
