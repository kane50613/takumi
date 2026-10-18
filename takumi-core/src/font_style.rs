use std::{
  borrow::Cow,
  cell::RefCell,
  hash::{Hash, Hasher},
  mem::discriminant,
  rc::Rc,
};

use parley::{
  FontFamily as ParleyFontFamily, FontFamilyName, FontFeatures, FontVariations, GenericFamily,
  LineBreak as ParleyLineBreak, LineHeight, TextStyle, VerticalAlign as ParleyVerticalAlign,
  WhiteSpaceCollapse as ParleyWhiteSpaceCollapse, fontique::QueryFamily,
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
    AppliedTextDecorations, Color, ComputedStyle, Display, FontFamily, FontFeature, FontSynthesis,
    FontVariantCaps, FontVariation, Lang, LineHeight as CssLineHeight, ResolvedFontVariations,
    SizingContext, WordBreak,
  },
};

/// A `font-family` stack in the form parley queries.
#[derive(Clone)]
pub(crate) struct ExpandedFontFamily {
  tokens: Vec<ExpandedFamilyToken>,
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

#[derive(Clone, PartialEq)]
enum ExpandedFamilyToken {
  Named(String),
  Generic(GenericFamily),
}

impl ExpandedFontFamily {
  fn new(tokens: Vec<ExpandedFamilyToken>) -> Self {
    let mut hasher = Xxh3::new();

    for token in &tokens {
      match token {
        ExpandedFamilyToken::Named(name) => {
          0_u8.hash(&mut hasher);
          name.hash(&mut hasher);
        }
        ExpandedFamilyToken::Generic(generic) => {
          1_u8.hash(&mut hasher);
          (*generic as u8).hash(&mut hasher);
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

  fn iter(&self) -> impl Iterator<Item = FontFamilyName<'_>> + Clone {
    self.tokens.iter().map(|token| match token {
      ExpandedFamilyToken::Named(name) => FontFamilyName::Named(name.as_str().into()),
      ExpandedFamilyToken::Generic(generic) => FontFamilyName::Generic(*generic),
    })
  }

  pub(crate) fn query_families(&self) -> impl Iterator<Item = QueryFamily<'_>> + Clone {
    self.tokens.iter().map(|token| match token {
      ExpandedFamilyToken::Named(name) => QueryFamily::Named(name.as_str()),
      ExpandedFamilyToken::Generic(generic) => QueryFamily::Generic(*generic),
    })
  }

  fn to_parley(&self) -> ParleyFontFamily<'_> {
    ParleyFontFamily::List(self.iter().collect())
  }

  pub(crate) fn expand(family: &FontFamily) -> Self {
    Self::new(
      family
        .names()
        .map(|name| match name {
          FontFamilyName::Named(name) => ExpandedFamilyToken::Named(name.into_owned()),
          FontFamilyName::Generic(generic) => ExpandedFamilyToken::Generic(generic),
        })
        .collect(),
    )
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
  pub(crate) font_variations: ResolvedFontVariations,
  /// The `font-variant-caps` feature to shape with, which a synthesized caps run swaps.
  pub(crate) caps: FontVariantCaps,
  pub(crate) line_height: parley::LineHeight,
  /// The line height kept on the brush, because parley's run metrics can carry a neighboring
  /// span's style at run boundaries.
  pub(crate) box_line_height: BoxLineHeight,
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
      is_direction_mark: false,
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
      font_synthesis: FontSynthesis {
        weight: self.parent.inherited_data.font_synthesis_weight,
        style: self.parent.inherited_data.font_synthesis_style,
        small_caps: self.parent.inherited_data.font_synthesis_small_caps,
      },
      font_variations: self.font_variations.clone(),
      line_height: self.box_line_height,
      fixed_letter_spacing: if self.parent.inherited_data.letter_spacing.has_percentage() {
        0.0
      } else {
        self.letter_spacing
      },
      fixed_word_spacing: if self.parent.inherited_data.word_spacing.has_percentage() {
        0.0
      } else {
        self.word_spacing
      },
      vertical_align: self.parent.base_data.vertical_align,
    }
  }

  /// The `text-shadow` layers in painting order: back to front, so the first
  /// one authored ends up on top, and without the ones nobody would see.
  ///
  /// Blink walks its shadow vector the same way in `TextPainter`.
  pub fn painted_text_shadows(&self) -> impl Iterator<Item = &SizedShadow> {
    self
      .text_shadow
      .iter()
      .rev()
      .filter(|shadow| shadow.color.0[3] != 0)
  }

  /// OpenType features to shape with, after `letter-spacing` has disabled ligatures.
  fn font_features(&self) -> Cow<'_, [FontFeature]> {
    self
      .parent
      .resolved_font_features(self.letter_spacing != 0.0, self.caps)
  }
}

impl<'s> From<&'s SizedFontStyle<'s>> for TextStyle<'s, 's, InlineBrush> {
  fn from(style: &'s SizedFontStyle<'s>) -> Self {
    TextStyle {
      font_size: style.sizing.font_size,
      line_height: style.line_height,
      font_weight: style.parent.inherited_data.font_weight.into_parlance(),
      font_style: style.parent.inherited_data.font_style.into_parlance(),
      font_variations: FontVariations::List(Cow::Owned(
        style
          .font_variations
          .iter()
          .map(FontVariation::into_parlance)
          .collect(),
      )),
      font_features: FontFeatures::List(Cow::Owned(
        style
          .font_features()
          .iter()
          .map(|feature| feature.into_parlance())
          .collect(),
      )),
      font_family: style.font_family.to_parley(),
      letter_spacing: style.letter_spacing,
      word_spacing: style.word_spacing,
      word_break: style.parent.misc_inherited_data.word_break.into_parley(),
      overflow_wrap: if style.parent.misc_inherited_data.word_break == WordBreak::BreakWord {
        // When word-break is break-word, ignore the overflow-wrap property's value.
        // https://developer.mozilla.org/en-US/docs/Web/CSS/word-break#break-word
        parley::OverflowWrap::Anywhere
      } else {
        style.parent.misc_inherited_data.overflow_wrap.into_parley()
      },
      brush: style.brush(),
      text_wrap_mode: style.parent.resolved_text_wrap_mode().into_parley(),
      line_break: ParleyLineBreak::default(),
      // Inline layout collapses white space and aligns boxes itself, as Blink's inline items do.
      white_space_collapse: ParleyWhiteSpaceCollapse::Preserve,
      vertical_align: ParleyVerticalAlign::BASELINE,
      font_width: style.parent.inherited_data.font_stretch.into_parlance(),

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
  /// The line height a box styled `style` keeps on its brush, as [`SizedFontStyle`] resolves it.
  pub(crate) fn of(style: &ComputedStyle, context: &RenderContext) -> Self {
    let css = style.inherited_data.line_height;

    if matches!(css, CssLineHeight::Normal) {
      return Self::Normal;
    }
    match layout_unit_line_height(css.into_parley(&context.sizing), context.sizing.font_size) {
      LineHeight::Absolute(value) => Self::Length(LayoutUnit::from_float(value)),
      LineHeight::FontSizeRelative(_) | LineHeight::MetricsRelative(_) => match css {
        CssLineHeight::Unitless(value) => Self::Number {
          value,
          font_size: context.sizing.font_size,
        },
        _ => Self::Normal,
      },
    }
  }

  /// Blink's `ComputedLineHeightAsFixed` against `font`, whose size a number multiplies.
  pub(crate) fn computed_as_fixed(self, font: &Font) -> LayoutUnit {
    match self {
      Self::Length(line_height) => line_height,
      Self::Normal => font
        .primary_font_metrics()
        .map_or(LayoutUnit::ZERO, PrimaryFontMetrics::fixed_line_spacing),
      Self::Number { value, .. } => number_line_height(value, font.description.computed_size),
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

/// `line_height` in layout units, as Blink's `ComputedLineHeightAsFixed` resolves it: a length
/// rounded to the nearest unit, and a number as `number_line_height`.
fn layout_unit_line_height(line_height: LineHeight, font_size: f32) -> LineHeight {
  match line_height {
    LineHeight::Absolute(value) => {
      LineHeight::Absolute(LayoutUnit::from_float_round(value).to_float())
    }
    LineHeight::FontSizeRelative(value) if font_size > 0.0 => {
      LineHeight::FontSizeRelative(number_line_height(value, font_size).to_float() / font_size)
    }
    line_height => line_height,
  }
}

impl<'s> SizedFontStyle<'s> {
  /// Resolves a sized font style from a computed style and render context.
  pub fn from_style(style: &'s ComputedStyle, context: &RenderContext) -> Self {
    let line_height_is_normal = matches!(style.inherited_data.line_height, CssLineHeight::Normal);
    let line_height = if line_height_is_normal {
      LineHeight::Absolute(context.resolve_normal_line_height(style, context.sizing.font_size))
    } else {
      layout_unit_line_height(
        style
          .inherited_data
          .line_height
          .into_parley(&context.sizing),
        context.sizing.font_size,
      )
    };

    let box_line_height = BoxLineHeight::of(style, context);

    Self {
      sizing: context.sizing.to_owned(),
      parent: style,
      font_family: context.expand_font_family(&style.inherited_data.font_family),
      font_variations: ResolvedFontVariations::new(
        style.inherited_data.font_variation_settings.clone(),
        style.inherited_data.font_optical_sizing,
        context.sizing.to_css(context.sizing.font_size),
      ),
      caps: style.inherited_data.font_variant_caps,
      line_height,
      box_line_height,
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

// Weight/stretch/style nearest-match face selection lives in parley's fontique query engine,
// not in this file; the one testable seam owned by this module is `ExpandedFontFamily::expand`.
#[cfg(test)]
mod tests {
  use parley::{FontFamilyName, GenericFamily};

  use super::{ExpandedFamilyToken, ExpandedFontFamily};
  use crate::style::{FontFamily, FromCssStr};

  #[test]
  fn family_token_hashes_tell_a_generic_from_a_name() {
    use std::hash::{DefaultHasher, Hasher};

    let hash = |tokens: Vec<ExpandedFamilyToken>| {
      let mut hasher = DefaultHasher::new();
      ExpandedFontFamily::new(tokens).hash_tokens(&mut hasher);
      hasher.finish()
    };

    assert_ne!(
      hash(vec![
        ExpandedFamilyToken::Generic(GenericFamily::SansSerif),
        ExpandedFamilyToken::Named(String::new()),
      ]),
      hash(vec![ExpandedFamilyToken::Named("\u{1}".to_owned())])
    );
  }

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
  fn names_pass_through_unchanged() {
    let family = FontFamily::from_css_str("Geist, serif").unwrap();
    let expanded = ExpandedFontFamily::expand(&family);

    assert_eq!(expanded.tokens.len(), 2);
    assert_eq!(
      names(&expanded),
      vec!["Geist".to_string(), "Serif".to_string()]
    );
  }

  #[test]
  fn generic_family_tokens_pass_through_expansion() {
    let family = FontFamily::from_css_str("monospace").unwrap();
    let expanded = ExpandedFontFamily::expand(&family);

    assert_eq!(expanded.tokens.len(), 1);
    assert!(matches!(
      expanded.iter().next(),
      Some(FontFamilyName::Generic(GenericFamily::Monospace))
    ));
  }
}
