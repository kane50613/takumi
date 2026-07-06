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
    BorderStyle, Color, ComputedStyle, Display, FontFamily, FontSynthesis, Length,
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

impl<'s> From<&'s SizedFontStyle<'s>> for TextStyle<'s, 's, InlineBrush> {
  fn from(style: &'s SizedFontStyle<'s>) -> Self {
    TextStyle {
      font_size: style.sizing.font_size,
      line_height: style.line_height,
      font_weight: style.parent.font_weight.into(),
      font_style: style.parent.font_style.into(),
      font_variations: FontVariations::List(Cow::Borrowed(
        style.parent.font_variation_settings.as_ref(),
      )),
      font_features: FontFeatures::List(style.parent.resolved_font_features()),
      font_family: style.font_family.to_parley(),
      letter_spacing: style.letter_spacing,
      word_spacing: style.word_spacing,
      word_break: style.parent.word_break.into(),
      overflow_wrap: if style.parent.word_break == WordBreak::BreakWord {
        // When word-break is break-word, ignore the overflow-wrap property's value.
        // https://developer.mozilla.org/en-US/docs/Web/CSS/word-break#break-word
        parley::OverflowWrap::Anywhere
      } else {
        style.parent.overflow_wrap.into()
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
      text_wrap_mode: style.parent.resolved_text_wrap_mode().into(),
      font_width: style.parent.font_stretch.into(),

      locale: style.parent.lang,
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
