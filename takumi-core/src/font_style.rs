use std::{
  borrow::Cow,
  collections::{BTreeSet, HashMap},
};

use parley::{
  FontFamily as ParleyFontFamily, FontFamilyName, FontFeatures, FontVariations, GenericFamily,
  TextStyle, fontique::QueryFamily,
};
use smallvec::SmallVec;

use crate::{
  context::RenderContext,
  geometry::Size,
  layout::inline::InlineBrush,
  shadow::SizedShadow,
  style::{
    BorderStyle, Color, ComputedStyle, Display, FontFamily, FontSynthesis, Lang, Length,
    SizedTextDecorationThickness, SizingContext, WordBreak,
  },
};

/// A `font-family` after subset-group expansion: each authored family name that names a
/// subset group (see [`crate::resources::font::FontResource::subset_of`]) is replaced by
/// its registered subset families, in order, so the shaper's primary chain carries every
/// coverage variant. Names that are not subset groups pass through unchanged.
#[derive(Clone, Default)]
pub(crate) struct ExpandedFontFamily(Vec<ExpandedFamilyToken>);

#[derive(Clone)]
enum ExpandedFamilyToken {
  Named(String),
  Generic(GenericFamily),
}

impl ExpandedFontFamily {
  fn iter(&self) -> impl Iterator<Item = FontFamilyName<'_>> + Clone {
    self.0.iter().map(|token| match token {
      ExpandedFamilyToken::Named(name) => FontFamilyName::Named(name.as_str().into()),
      ExpandedFamilyToken::Generic(generic) => FontFamilyName::Generic(*generic),
    })
  }

  pub(crate) fn query_families(&self) -> impl Iterator<Item = QueryFamily<'_>> + Clone {
    self.0.iter().map(|token| match token {
      ExpandedFamilyToken::Named(name) => QueryFamily::Named(name.as_str()),
      ExpandedFamilyToken::Generic(generic) => QueryFamily::Generic(*generic),
    })
  }

  fn to_parley(&self) -> ParleyFontFamily<'_> {
    ParleyFontFamily::List(self.iter().collect())
  }

  /// Expands `family` against the registered subset `groups`: a name that's a subset group
  /// becomes its registered subset families (in order); other names pass through unchanged.
  fn expand(family: &FontFamily, groups: &HashMap<String, BTreeSet<String>>) -> Self {
    let mut tokens = Vec::new();
    for name in family.names() {
      match name {
        FontFamilyName::Named(name) => match groups.get(name.as_ref()) {
          Some(subsets) => {
            tokens.extend(
              subsets
                .iter()
                .map(|s| ExpandedFamilyToken::Named(s.clone())),
            );
          }
          None => tokens.push(ExpandedFamilyToken::Named(name.into_owned())),
        },
        FontFamilyName::Generic(generic) => tokens.push(ExpandedFamilyToken::Generic(generic)),
      }
    }
    Self(tokens)
  }
}

impl RenderContext {
  pub(crate) fn expand_font_family(&self, family: &FontFamily) -> ExpandedFontFamily {
    ExpandedFontFamily::expand(family, &self.fonts.groups)
  }
}

/// Sized font style with computed font size and line height.
#[derive(Clone)]
#[non_exhaustive]
pub struct SizedFontStyle<'s> {
  /// Computed style this is derived from.
  pub parent: &'s ComputedStyle,
  pub(crate) font_family: ExpandedFontFamily,
  pub(crate) line_height: parley::LineHeight,
  pub(crate) line_height_scales_with_text_fit: bool,
  /// Text stroke width in pixels.
  pub stroke_width: f32,
  /// Outline width in pixels.
  pub outline_width: f32,
  /// Gap between outline and border edge, in pixels.
  pub outline_offset: f32,
  pub(crate) letter_spacing: f32,
  pub(crate) word_spacing: f32,
  /// Resolved text shadows.
  pub text_shadow: SmallVec<[SizedShadow; 4]>,
  pub(crate) color: Color,
  /// Outline color.
  pub outline_color: Color,
  /// Outline line style.
  pub outline_style: BorderStyle,
  /// Text stroke color.
  pub text_stroke_color: Color,
  pub(crate) text_decoration_color: Color,
  pub(crate) text_decoration_thickness: SizedTextDecorationThickness,
  pub(crate) text_underline_offset: f32,
  /// Resolved sizing context (font size, etc.).
  pub sizing: SizingContext,
}

/// Feeds a [`fmt::Debug`] stream straight into a hasher, so style fields hash
/// without intermediate strings.
struct HashWriter<'a, H: core::hash::Hasher>(&'a mut H);

impl<H: core::hash::Hasher> core::fmt::Write for HashWriter<'_, H> {
  fn write_str(&mut self, s: &str) -> core::fmt::Result {
    self.0.write(s.as_bytes());
    Ok(())
  }
}

impl SizedFontStyle<'_> {
  /// Hashes every input the `TextStyle` conversion below reads, so shaped
  /// text-only layouts can be cached by content. Keep in sync with
  /// `From<&SizedFontStyle> for TextStyle`.
  pub(crate) fn hash_shaping_inputs(&self, hasher: &mut impl core::hash::Hasher) {
    use core::fmt::Write;
    use core::hash::Hash;

    self.sizing.font_size.to_bits().hash(hasher);
    self.letter_spacing.to_bits().hash(hasher);
    self.word_spacing.to_bits().hash(hasher);
    self.text_underline_offset.to_bits().hash(hasher);
    self.line_height_scales_with_text_fit.hash(hasher);
    self.color.0.hash(hasher);
    self.text_decoration_color.0.hash(hasher);
    self.text_stroke_color.0.hash(hasher);
    for token in &self.font_family.0 {
      match token {
        ExpandedFamilyToken::Named(name) => name.hash(hasher),
        ExpandedFamilyToken::Generic(generic) => (*generic as u8).hash(hasher),
      }
    }

    let parent = self.parent;
    let mut writer = HashWriter(hasher);

    let _ = write!(
      writer,
      "{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}",
      self.line_height,
      self.text_decoration_thickness,
      parent.font_weight,
      parent.font_style,
      parent.font_stretch,
      parent.font_variation_settings,
      parent.resolved_font_features(),
      parent.word_break,
      parent.overflow_wrap,
      parent.display,
      parent.opacity,
      parent.text_underline_position,
      parent.text_decoration_line,
      parent.text_decoration_skip_ink,
      parent.font_synthesis_weight,
      parent.font_synthesis_style,
      parent.vertical_align,
      parent.resolved_text_wrap_mode(),
    );
    parent.lang.as_ref().map(Lang::as_str).hash(hasher);
  }
}

impl<'s> From<&'s SizedFontStyle<'s>> for TextStyle<'s, 's, InlineBrush> {
  fn from(style: &'s SizedFontStyle<'s>) -> Self {
    TextStyle {
      font_size: style.sizing.font_size,
      line_height: style.line_height,
      font_weight: style.parent.font_weight.into_parlance(),
      font_style: style.parent.font_style.into_parlance(),
      font_variations: FontVariations::List(Cow::Owned(
        style
          .parent
          .font_variation_settings
          .iter()
          .map(|variation| variation.into_parlance())
          .collect(),
      )),
      font_features: FontFeatures::List(Cow::Owned(
        style
          .parent
          .resolved_font_features()
          .iter()
          .map(|feature| feature.into_parlance())
          .collect(),
      )),
      font_family: style.font_family.to_parley(),
      letter_spacing: style.letter_spacing,
      word_spacing: style.word_spacing,
      word_break: style.parent.word_break.into_parley(),
      overflow_wrap: if style.parent.word_break == WordBreak::BreakWord {
        // When word-break is break-word, ignore the overflow-wrap property's value.
        // https://developer.mozilla.org/en-US/docs/Web/CSS/word-break#break-word
        parley::OverflowWrap::Anywhere
      } else {
        style.parent.overflow_wrap.into_parley()
      },
      brush: InlineBrush {
        source_span_id: None,
        // Inline elements don't establish a stacking context, so we handle opacity here.
        opacity: if style.parent.display == Display::Inline {
          style.parent.opacity.0
        } else {
          1.0
        },
        color: style.color,
        decoration_color: style.text_decoration_color,
        decoration_thickness: style.text_decoration_thickness,
        underline_offset: style.text_underline_offset,
        underline_position: style.parent.text_underline_position,
        decoration_line: style.parent.text_decoration_line.unwrap_or_default(),
        decoration_skip_ink: style.parent.text_decoration_skip_ink,
        stroke_color: style.text_stroke_color,
        font_synthesis: FontSynthesis {
          weight: style.parent.font_synthesis_weight,
          style: style.parent.font_synthesis_style,
        },
        line_height_scales_with_text_fit: style.line_height_scales_with_text_fit,
        vertical_align: style.parent.vertical_align,
      },
      text_wrap_mode: style.parent.resolved_text_wrap_mode().into_parley(),
      font_width: style.parent.font_stretch.into_parlance(),

      locale: style.parent.lang.map(Lang::into_parlance),
      has_underline: false,
      underline_offset: None,
      underline_size: None,
      underline_brush: None,
      has_strikethrough: false,
      strikethrough_offset: None,
      strikethrough_size: None,
      strikethrough_brush: None,
    }
  }
}

#[inline]
fn resolved_text_shadows(
  style: &ComputedStyle,
  context: &RenderContext,
) -> SmallVec<[SizedShadow; 4]> {
  style
    .text_shadow
    .as_ref()
    .map_or_else(SmallVec::new, |shadows| {
      shadows
        .iter()
        .map(|shadow| {
          SizedShadow::from_text_shadow(
            *shadow,
            &context.sizing,
            context.current_color,
            Size::new(context.sizing.font_size, context.sizing.font_size),
          )
        })
        .collect()
    })
}

impl<'s> SizedFontStyle<'s> {
  /// Resolves a sized font style from a computed style and render context.
  pub fn from_style(style: &'s ComputedStyle, context: &RenderContext) -> Self {
    let line_height = style.line_height.into_parley(&context.sizing);

    Self {
      sizing: context.sizing.to_owned(),
      parent: style,
      font_family: context.expand_font_family(&style.font_family),
      line_height,
      line_height_scales_with_text_fit: style.line_height.scales_with_text_fit(),
      stroke_width: style
        .webkit_text_stroke_width
        .unwrap_or_default()
        .to_px(&context.sizing, context.sizing.font_size),
      // Outline is not inherited; a non-inline element paints its outline on its
      // own border-box (see `draw_outline`), so only a real inline box strokes its
      // text fragments. https://www.w3.org/TR/css-ui-4/#outline
      outline_width: if style.display == Display::Inline {
        Length::from(style.outline_width)
          .to_px(&context.sizing, 0.0)
          .max(0.0)
      } else {
        0.0
      },
      outline_offset: style.outline_offset.to_px(&context.sizing, 0.0),
      letter_spacing: style
        .letter_spacing
        .to_px(&context.sizing, context.sizing.font_size),
      word_spacing: style
        .word_spacing
        .to_px(&context.sizing, context.sizing.font_size),
      text_shadow: resolved_text_shadows(style, context),
      color: style
        .webkit_text_fill_color
        .unwrap_or(style.color)
        .resolve(context.current_color),
      outline_color: style.outline_color.resolve(context.current_color),
      outline_style: style.outline_style,
      text_stroke_color: style
        .webkit_text_stroke_color
        .unwrap_or_default()
        .resolve(context.current_color),
      text_decoration_color: style.text_decoration_color.resolve(context.current_color),
      text_decoration_thickness: style.resolved_text_decoration_thickness(&context.sizing),
      text_underline_offset: style.text_underline_offset.resolve_px(&context.sizing),
    }
  }
}

// Weight/stretch/style nearest-match face selection lives in parley's fontique query engine,
// not in this file; the one testable seam owned by this module is `ExpandedFontFamily::expand`.
#[cfg(test)]
mod tests {
  use std::collections::{BTreeSet, HashMap};

  use parley::{FontFamilyName, GenericFamily};

  use super::ExpandedFontFamily;
  use crate::style::{FontFamily, FromCssStr};

  fn names(expanded: &ExpandedFontFamily) -> Vec<String> {
    expanded
      .iter()
      .map(|name| match name {
        FontFamilyName::Named(name) => name.into_owned(),
        FontFamilyName::Generic(generic) => format!("{generic:?}"),
      })
      .collect()
  }

  #[test]
  fn names_not_in_groups_pass_through_unchanged() {
    let family = FontFamily::from_css_str("Geist, serif").unwrap();
    let expanded = ExpandedFontFamily::expand(&family, &HashMap::new());

    assert_eq!(expanded.0.len(), 2);
    assert_eq!(
      names(&expanded),
      vec!["Geist".to_string(), "Serif".to_string()]
    );
  }

  #[test]
  fn logical_subset_group_expands_to_its_registered_subsets_in_order() {
    let family = FontFamily::from_css_str("Logical").unwrap();
    let mut groups = HashMap::new();
    groups.insert(
      "Logical".to_string(),
      BTreeSet::from(["Subset A".to_string(), "Subset B".to_string()]),
    );

    let expanded = ExpandedFontFamily::expand(&family, &groups);

    assert_eq!(
      names(&expanded),
      vec!["Subset A".to_string(), "Subset B".to_string()]
    );
  }

  #[test]
  fn generic_family_tokens_pass_through_expansion() {
    let family = FontFamily::from_css_str("monospace").unwrap();
    let expanded = ExpandedFontFamily::expand(&family, &HashMap::new());

    assert_eq!(expanded.0.len(), 1);
    assert!(matches!(
      expanded.iter().next(),
      Some(FontFamilyName::Generic(GenericFamily::Monospace))
    ));
  }
}
