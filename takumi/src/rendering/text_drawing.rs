use std::{borrow::Cow, ops::Range};

use image::RgbaImage;
use parley::{Glyph, GlyphRun};
use taffy::{Layout, Point, Size};
use zeno::{Command, Join, Mask, PathData, Placement, Stroke};

use crate::{
  GlobalContext,
  layout::{
    inline::{InlineBrush, break_lines},
    style::{
      Affine, Color, ImageScalingAlgorithm, SizedFontStyle, TextTransform, WhiteSpaceCollapse,
    },
  },
  rendering::{BorderProperties, Canvas, apply_mask_alpha_to_pixel},
  resources::font::ResolvedGlyph,
};

pub(crate) fn draw_decoration(
  canvas: &mut Canvas,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  color: Color,
  offset: f32,
  size: f32,
  layout: Layout,
  transform: Affine,
) {
  let transform = Affine::translation(Size {
    width: layout.border.left + layout.padding.left + glyph_run.offset(),
    height: layout.border.top + layout.padding.top + offset,
  }) * transform;

  canvas.fill_color(
    Point {
      x: layout.location.x as i32,
      y: layout.location.y as i32,
    },
    Size {
      width: glyph_run.advance().round() as u32,
      height: size.round() as u32,
    },
    color,
    BorderProperties::default(),
    transform,
  );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_glyph(
  glyph: Glyph,
  glyph_content: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  layout: Layout,
  image_fill: Option<&RgbaImage>,
  transform: Affine,
  text_style: &parley::Style<InlineBrush>,
) {
  let transform = Affine::translation(Size {
    width: layout.border.left + layout.padding.left + glyph.x,
    height: layout.border.top + layout.padding.top + glyph.y,
  }) * transform;

  if let ResolvedGlyph::Image(bitmap) = glyph_content {
    let border = BorderProperties {
      size: Size {
        width: bitmap.placement.width as f32,
        height: bitmap.placement.height as f32,
      },
      ..Default::default()
    };

    let offset = Point {
      x: layout.location.x as i32 + bitmap.placement.left,
      y: layout.location.y as i32 - bitmap.placement.top,
    };

    if let Some(image_fill) = image_fill {
      let mask = bitmap
        .data
        .iter()
        .skip(3)
        .step_by(4)
        .copied()
        .collect::<Vec<_>>();

      let placement = Placement {
        left: 0,
        top: 0,
        width: bitmap.placement.width,
        height: bitmap.placement.height,
      };

      let mut bottom = RgbaImage::new(placement.width, placement.height);

      let mut i = 0;

      for y in 0..placement.height {
        for x in 0..placement.width {
          let alpha = mask[i];
          i += 1;

          if alpha == 0 {
            continue;
          }

          let source_x = x + glyph.x as u32;
          let source_y = y + glyph.y as u32 - bitmap.placement.top as u32;

          let Some(pixel) = image_fill.get_pixel_checked(source_x, source_y) else {
            continue;
          };

          let pixel = apply_mask_alpha_to_pixel(*pixel, alpha);

          bottom.put_pixel(x, y, pixel);
        }
      }

      return canvas.overlay_image(
        &bottom,
        offset,
        border,
        transform,
        ImageScalingAlgorithm::Auto,
        None,
      );
    }

    let image = RgbaImage::from_raw(
      bitmap.placement.width,
      bitmap.placement.height,
      bitmap.data.clone(),
    )
    .unwrap();

    return canvas.overlay_image(
      &image,
      offset,
      border,
      transform,
      ImageScalingAlgorithm::Auto,
      None,
    );
  }

  if let ResolvedGlyph::Outline(outline) = glyph_content {
    // have to invert the y coordinate from y-up to y-down first
    let mut paths = outline
      .path()
      .commands()
      .map(|command| match command {
        Command::MoveTo(point) => Command::MoveTo((point.x, -point.y).into()),
        Command::LineTo(point) => Command::LineTo((point.x, -point.y).into()),
        Command::CurveTo(point1, point2, point3) => Command::CurveTo(
          (point1.x, -point1.y).into(),
          (point2.x, -point2.y).into(),
          (point3.x, -point3.y).into(),
        ),
        Command::QuadTo(point1, point2) => {
          Command::QuadTo((point1.x, -point1.y).into(), (point2.x, -point2.y).into())
        }
        Command::Close => Command::Close,
      })
      .collect::<Vec<_>>();

    transform.apply_on_paths(&mut paths);

    let (mask, mut placement) = Mask::new(&paths).render();

    if let Some(ref shadows) = style.text_shadow {
      for shadow in shadows.iter() {
        shadow.draw_outset(canvas, Cow::Borrowed(&mask), placement, layout.location);
      }
    }

    let cropped_fill_image = image_fill.map(|image| {
      let mut bottom = RgbaImage::new(placement.width, placement.height);

      for y in 0..placement.height {
        let dest_y = y + placement.top as u32;

        if dest_y >= image.height() {
          continue;
        }

        for x in 0..placement.width {
          let dest_x = x + placement.left as u32;

          if dest_x >= image.width() {
            continue;
          }

          bottom.put_pixel(x, y, *image.get_pixel(dest_x, dest_y));
        }
      }

      bottom
    });

    placement.left += layout.location.x as i32;
    placement.top += layout.location.y as i32;

    canvas.draw_mask(&mask, placement, text_style.brush.color, cropped_fill_image);

    if style.stroke_width > 0.0 {
      let mut stroke = Stroke::new(style.stroke_width);
      stroke.scale = false;
      stroke.join = Join::Bevel;

      let (stroke_mask, mut stroke_placement) = Mask::new(&paths).style(stroke).render();

      stroke_placement.left += layout.location.x as i32;
      stroke_placement.top += layout.location.y as i32;

      canvas.draw_mask(
        &stroke_mask,
        stroke_placement,
        style.text_stroke_color,
        None,
      );
    }
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum MaxHeight {
  Absolute(f32),
  Lines(u32),
  HeightAndLines(f32, u32),
}

/// Applies text transform to the input text.
pub(crate) fn apply_text_transform<'a>(input: &'a str, transform: TextTransform) -> Cow<'a, str> {
  match transform {
    TextTransform::None => Cow::Borrowed(input),
    TextTransform::Uppercase => Cow::Owned(input.to_uppercase()),
    TextTransform::Lowercase => Cow::Owned(input.to_lowercase()),
    TextTransform::Capitalize => {
      let mut result = String::with_capacity(input.len());
      let mut start_of_word = true;
      for ch in input.chars() {
        if ch.is_alphabetic() {
          if start_of_word {
            result.extend(ch.to_uppercase());
            start_of_word = false;
          } else {
            result.extend(ch.to_lowercase());
          }
        } else {
          start_of_word = !ch.is_numeric();
          result.push(ch);
        }
      }
      Cow::Owned(result)
    }
  }
}

/// Applies whitespace collapse rules to the input text according to `WhiteSpaceCollapse`.
pub(crate) fn apply_white_space_collapse<'a>(
  input: &'a str,
  collapse: WhiteSpaceCollapse,
) -> Cow<'a, str> {
  match collapse {
    WhiteSpaceCollapse::Preserve => Cow::Borrowed(input),

    // Collapse sequences of whitespace (spaces, tabs, line breaks) into a single space
    // and trim leading/trailing spaces.
    WhiteSpaceCollapse::Collapse => {
      let mut out = String::with_capacity(input.len());
      let mut last_was_ws = false;

      for ch in input.chars() {
        if ch.is_whitespace() {
          if !last_was_ws {
            out.push(' ');
            last_was_ws = true;
          }
        } else {
          out.push(ch);
          last_was_ws = false;
        }
      }

      Cow::Owned(out.trim().to_string())
    }

    // Preserve sequences of spaces/tabs but remove line breaks (replace them with a single space).
    WhiteSpaceCollapse::PreserveSpaces => {
      let mut out = String::with_capacity(input.len());
      let mut last_was_space = false;

      for ch in input.chars() {
        // treat common line break characters as breaks to be removed/replaced
        if matches!(ch, '\n' | '\r' | '\x0B' | '\x0C' | '\u{2028}' | '\u{2029}') {
          if !last_was_space {
            out.push(' ');
            last_was_space = true;
          }
        } else {
          out.push(ch);
          last_was_space = ch == ' ' || ch == '\t';
        }
      }

      Cow::Owned(out)
    }

    // Preserve line breaks but collapse consecutive spaces and tabs into single spaces.
    // Also remove leading spaces after line breaks.
    WhiteSpaceCollapse::PreserveBreaks => {
      let mut out = String::with_capacity(input.len());
      let mut last_was_space = false;
      let mut last_was_line_break = false;

      for ch in input.chars() {
        if ch == ' ' || ch == '\t' {
          // Skip leading spaces after line breaks
          if last_was_line_break {
            continue;
          }
          if !last_was_space {
            out.push(' ');
            last_was_space = true;
          }
        } else {
          out.push(ch);
          last_was_space = false;
          // Track if we just processed a line break
          last_was_line_break =
            matches!(ch, '\n' | '\r' | '\x0B' | '\x0C' | '\u{2028}' | '\u{2029}');
        }
      }

      Cow::Owned(out.trim().to_string())
    }
  }
}

/// Construct a new string with an ellipsis appended such that it fits within `max_width`.
pub(crate) fn make_ellipsis_text<'s>(
  render_text: &'s str,
  text_range: Range<usize>,
  font_style: &SizedFontStyle,
  global: &GlobalContext,
  max_width: f32,
  ellipsis_char: &'s str,
) -> Cow<'s, str> {
  let mut truncated_text = &render_text[text_range.start..text_range.end];

  while !truncated_text.is_empty() {
    // try to calculate the last line only with the truncated text and ellipsis character
    let mut text_with_ellipsis = String::with_capacity(truncated_text.len() + ellipsis_char.len());

    text_with_ellipsis.push_str(truncated_text);
    text_with_ellipsis.push_str(ellipsis_char);

    let (mut inline_layout, _) = global
      .font_context
      .tree_builder(font_style.into(), |builder| {
        builder.push_text(&text_with_ellipsis);
      });

    break_lines(
      &mut inline_layout,
      max_width,
      Some(MaxHeight::Lines(2)),
      font_style.parent.white_space().text_wrap_mode,
    );

    // if the text fits, return the text with ellipsis character
    if inline_layout.lines().count() == 1 {
      let before_last_line = &render_text[..text_range.start];

      // build the text with ellipsis character
      let mut text_with_ellipsis =
        String::with_capacity(before_last_line.len() + truncated_text.len() + ellipsis_char.len());

      text_with_ellipsis.push_str(before_last_line);
      text_with_ellipsis.push_str(truncated_text);
      text_with_ellipsis.push_str(ellipsis_char);

      return Cow::Owned(text_with_ellipsis);
    }

    // try to shrink by one char
    if let Some((char_idx, _)) = truncated_text.char_indices().last() {
      truncated_text = &truncated_text[..char_idx];
    } else {
      // the text is empty, break out
      break;
    }
  }

  // if there's nothing left, returns nothing
  Cow::Borrowed("")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_white_space_preserve() {
    let input = "  a \t b\n";
    let out = apply_white_space_collapse(input, WhiteSpaceCollapse::Preserve);
    assert_eq!(out, input);
  }

  #[test]
  fn test_white_space_collapse() {
    let input = "  a \n\t b  c\n\n ";
    let out = apply_white_space_collapse(input, WhiteSpaceCollapse::Collapse);
    assert_eq!(out, "a b c");
  }

  #[test]
  fn test_white_space_preserve_spaces() {
    let input = "a \n b";
    let out = apply_white_space_collapse(input, WhiteSpaceCollapse::PreserveSpaces);
    // line break should be replaced with a single space; existing spaces preserved
    assert_eq!(out, "a  b");
  }

  #[test]
  fn test_white_space_preserve_breaks() {
    let input = "a \n b\tc";
    let out = apply_white_space_collapse(input, WhiteSpaceCollapse::PreserveBreaks);
    // spaces and tabs collapsed to single space, line break preserved
    assert_eq!(out, "a \nb c");
  }
}
