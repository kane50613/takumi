use std::borrow::Cow;

use parley::{FontFeatures, FontVariations, TextStyle};
use smallvec::SmallVec;
use taffy::{Size, prelude::FromLength};

use crate::context::RenderContext;
use crate::layout::inline::InlineBrush;
use crate::layout::style::{
  BorderStyle, Color, ComputedStyle, Display, FontSynthesis, Length, SizedTextDecorationThickness,
  SizingContext, WordBreak,
};
use crate::shadow::SizedShadow;

/// Sized font style with computed font size and line height.
#[derive(Clone)]
#[non_exhaustive]
pub struct SizedFontStyle<'s> {
  pub parent: &'s ComputedStyle,
  pub(crate) line_height: parley::LineHeight,
  pub(crate) line_height_scales_with_text_fit: bool,
  pub stroke_width: f32,
  pub outline_width: f32,
  pub outline_offset: f32,
  pub(crate) letter_spacing: f32,
  pub(crate) word_spacing: f32,
  pub text_shadow: SmallVec<[SizedShadow; 4]>,
  pub(crate) color: Color,
  pub outline_color: Color,
  pub outline_style: BorderStyle,
  pub text_stroke_color: Color,
  pub(crate) text_decoration_color: Color,
  pub(crate) text_decoration_thickness: SizedTextDecorationThickness,
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
      font_features: FontFeatures::List(Cow::Borrowed(style.parent.font_feature_settings.as_ref())),
      font_family: (&style.parent.font_family).into(),
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

      locale: None,
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
            Size::from_length(context.sizing.font_size),
          )
        })
        .collect()
    })
}

impl<'s> SizedFontStyle<'s> {
  pub fn from_style(style: &'s ComputedStyle, context: &RenderContext) -> Self {
    let line_height = style.line_height.into_parley(&context.sizing);

    Self {
      sizing: context.sizing.to_owned(),
      parent: style,
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
    }
  }
}
