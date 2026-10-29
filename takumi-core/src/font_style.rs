use std::{
  cell::RefCell,
  hash::{Hash, Hasher},
  mem::discriminant,
  rc::Rc,
};

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use xxhash_rust::xxh3::Xxh3;

use crate::{
  context::RenderContext,
  fonts::font::Font,
  geometry::Size,
  layout::inline::InlineBrush,
  layout_unit::LayoutUnit,
  resources::font::PrimaryFontMetrics,
  shadow::SizedShadow,
  style::{
    AppliedTextDecorations, Color, ComputedStyle, Display, FontFamily, FontFamilyToken,
    FontVariantCaps, LineHeight as CssLineHeight, SizingContext,
  },
};

/// A `font-family` stack as font matching walks it.
#[derive(Clone)]
pub(crate) struct ExpandedFontFamily {
  tokens: Vec<FontFamilyToken>,
  /// The tokens hashed in order, so cache keys hash one word instead of every name.
  digest: u64,
}

impl PartialEq for ExpandedFontFamily {
  fn eq(&self, other: &Self) -> bool {
    self.digest == other.digest && self.tokens == other.tokens
  }
}

impl Default for ExpandedFontFamily {
  fn default() -> Self {
    Self::new(Vec::new())
  }
}

impl ExpandedFontFamily {
  pub(crate) fn new(tokens: Vec<FontFamilyToken>) -> Self {
    let mut hasher = Xxh3::new();

    for token in &tokens {
      match token {
        FontFamilyToken::Owned(name) => {
          0_u8.hash(&mut hasher);
          name.hash(&mut hasher);
        }
        FontFamilyToken::Generic(generic) => {
          1_u8.hash(&mut hasher);
          generic.hash(&mut hasher);
        }
      }
    }

    Self {
      tokens,
      digest: hasher.finish(),
    }
  }

  /// Hashes the family list in order, names and generics alike.
  pub(crate) fn hash_tokens(&self, hasher: &mut impl Hasher) {
    hasher.write_u64(self.digest);
  }

  /// The families, in declaration order.
  pub(crate) fn tokens(&self) -> &[FontFamilyToken] {
    &self.tokens
  }

  pub(crate) fn expand(family: &FontFamily) -> Self {
    Self::new(family.tokens().to_vec())
  }
}

/// Each `font-family` stack a render expanded, by the id of the stack it came from.
#[derive(Default)]
pub(crate) struct ExpandedFamilies(RefCell<FxHashMap<u64, Rc<ExpandedFontFamily>>>);

impl RenderContext {
  pub(crate) fn expand_font_family(&self, family: &FontFamily) -> Rc<ExpandedFontFamily> {
    let mut expanded = self.font_families().0.borrow_mut();
    let expanded = expanded
      .entry(family.id())
      .or_insert_with(|| Rc::new(ExpandedFontFamily::expand(family)));

    Rc::clone(expanded)
  }
}

/// Sized font style with computed font size and line height.
#[derive(Clone)]
#[non_exhaustive]
pub struct SizedFontStyle<'s> {
  /// Computed style this is derived from.
  pub parent: &'s ComputedStyle,
  pub(crate) font_family: Rc<ExpandedFontFamily>,
  /// The `font-variant-caps` feature to shape with, which a synthesized caps run swaps.
  pub(crate) caps: FontVariantCaps,
  /// Text stroke width in pixels.
  pub stroke_width: f32,
  pub(crate) letter_spacing: f32,
  pub(crate) word_spacing: f32,
  /// Resolved text shadows.
  pub text_shadow: SmallVec<[SizedShadow; 4]>,
  pub(crate) color: Color,
  /// Text stroke color.
  pub(crate) text_stroke_color: Color,
  pub(crate) text_decorations: AppliedTextDecorations,
  /// Resolved sizing context (font size, etc.).
  pub sizing: SizingContext,
}

impl SizedFontStyle<'_> {
  /// The paint attributes a run in this style carries.
  pub(crate) fn brush(&self) -> InlineBrush {
    InlineBrush {
      source_span_id: None,
      // Inline elements don't establish a stacking context, so we handle opacity here.
      opacity: if self.parent.base_data.display == Display::Inline {
        self.parent.svg_data.opacity.0
      } else {
        1.0
      },
      color: self.color,
      decorations: self.text_decorations.clone(),
      decoration_skip_ink: self.parent.misc_inherited_data.text_decoration_skip_ink,
      stroke_color: self.text_stroke_color,
      stroke_width: self.stroke_width,
    }
  }

  /// The visible `text-shadow` layers back to front, as Blink's `TextPainter` walks them.
  pub fn painted_text_shadows(&self) -> impl Iterator<Item = &SizedShadow> {
    self
      .text_shadow
      .iter()
      .rev()
      .filter(|shadow| shadow.color.0[3] != 0)
  }
}

#[inline]
fn resolved_text_shadows(
  style: &ComputedStyle,
  context: &RenderContext,
) -> SmallVec<[SizedShadow; 4]> {
  style
    .misc_inherited1_data
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

/// A box's `line-height` as Blink's `ComputedLineHeightAsFixed` resolves it against a font.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum BoxLineHeight {
  /// `normal`: the font's own line spacing.
  Normal,
  /// A length, rounded to the nearest layout unit.
  Length(LayoutUnit),
  /// A number times the font size.
  Number { value: f32, font_size: f32 },
}

impl BoxLineHeight {
  /// The line height of a box styled `style` in `context`.
  pub(crate) fn of(style: &ComputedStyle, context: &RenderContext) -> Self {
    let sizing = &context.sizing;

    match style.inherited_data.line_height {
      CssLineHeight::Normal => Self::Normal,
      CssLineHeight::Unitless(value) => Self::Number {
        value,
        font_size: sizing.font_size,
      },
      CssLineHeight::Length(length) => Self::Length(LayoutUnit::from_float(
        LayoutUnit::from_float_round(length.to_px(sizing, sizing.font_size)).to_float(),
      )),
    }
  }

  /// Blink's `ComputedLineHeightAsFixed` against `font`, whose size a number multiplies.
  pub(crate) fn computed_as_fixed(self, font: &Font) -> LayoutUnit {
    self.computed_with_metrics(
      font.primary_font_metrics(),
      font.description.fallback.computed_size,
    )
  }

  /// Blink's `ComputedLineHeightAsFixed` against a primary font with `metrics` at `font_size`.
  pub(crate) fn computed_with_metrics(
    self,
    metrics: Option<PrimaryFontMetrics>,
    font_size: f32,
  ) -> LayoutUnit {
    match self {
      Self::Length(line_height) => line_height,
      Self::Normal => metrics.map_or(LayoutUnit::ZERO, PrimaryFontMetrics::fixed_line_spacing),
      Self::Number { value, .. } => number_line_height(value, font_size),
    }
  }
}

impl Hash for BoxLineHeight {
  fn hash<H: Hasher>(&self, hasher: &mut H) {
    discriminant(self).hash(hasher);
    match *self {
      Self::Normal => {}
      Self::Length(line_height) => line_height.hash(hasher),
      Self::Number { value, font_size } => {
        value.to_bits().hash(hasher);
        font_size.to_bits().hash(hasher);
      }
    }
  }
}

/// A number line height, which Blink keeps as a percentage, truncated against the font size
/// rounded to a layout unit.
fn number_line_height(value: f32, font_size: f32) -> LayoutUnit {
  LayoutUnit::from_float(
    LayoutUnit::from_float_round(font_size).to_float() * (value * 100.0) / 100.0,
  )
}

impl<'s> SizedFontStyle<'s> {
  /// Resolves a sized font style from a computed style and render context.
  pub fn from_style(style: &'s ComputedStyle, context: &RenderContext) -> Self {
    Self {
      sizing: context.sizing.to_owned(),
      parent: style,
      font_family: context.expand_font_family(&style.inherited_data.font_family),
      caps: style.inherited_data.font_variant_caps,
      stroke_width: style
        .misc_inherited1_data
        .webkit_text_stroke_width
        .unwrap_or_default()
        .to_px(&context.sizing, context.sizing.font_size),
      letter_spacing: style
        .inherited_data
        .letter_spacing
        .to_px(&context.sizing, context.sizing.font_size),
      word_spacing: style
        .inherited_data
        .word_spacing
        .to_px(&context.sizing, context.sizing.font_size),
      text_shadow: resolved_text_shadows(style, context),
      color: style
        .misc_inherited1_data
        .webkit_text_fill_color
        .unwrap_or(style.inherited_data.color)
        .resolve(context.current_color),
      text_stroke_color: style
        .misc_inherited1_data
        .webkit_text_stroke_color
        .unwrap_or_default()
        .resolve(context.current_color),
      text_decorations: context.text_decorations.clone(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::ExpandedFontFamily;
  use crate::style::{FontFamily, FontFamilyToken, FromCssStr, GenericFamilyType};

  #[test]
  fn family_token_hashes_tell_a_generic_from_a_name() {
    use std::hash::{DefaultHasher, Hasher};

    let hash = |tokens: Vec<FontFamilyToken>| {
      let mut hasher = DefaultHasher::new();
      ExpandedFontFamily::new(tokens).hash_tokens(&mut hasher);
      hasher.finish()
    };

    assert_ne!(
      hash(vec![
        FontFamilyToken::Generic(GenericFamilyType::SansSerif),
        FontFamilyToken::Owned(String::new()),
      ]),
      hash(vec![FontFamilyToken::Owned("\u{1}".to_owned())])
    );
  }

  #[test]
  fn names_pass_through_unchanged() {
    let family = FontFamily::from_css_str("Geist, serif").unwrap();
    let expanded = ExpandedFontFamily::expand(&family);

    assert_eq!(
      expanded.tokens(),
      [
        FontFamilyToken::Owned("Geist".to_string()),
        FontFamilyToken::Generic(GenericFamilyType::Serif),
      ]
    );
  }
}
