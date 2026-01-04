use std::{borrow::Cow, convert::Into};

use image::{
  GenericImageView, ImageError, Rgba, RgbaImage,
  error::{DecodingError, ImageFormatHint},
  imageops::{interpolate_bilinear, interpolate_nearest},
};
use parley::{Glyph, GlyphRun};
use swash::{ColorPalette, scale::outline::Outline};
use taffy::{Layout, Point, Size};
use zeno::{Command, Join, PathData, Stroke};

use crate::{
  Result,
  layout::{
    inline::{InlineBrush, InlineLayout, break_lines},
    style::{
      Affine, Color, ImageScalingAlgorithm, SizedFontStyle, TextTransform, WhiteSpaceCollapse,
    },
  },
  rendering::{
    BorderProperties, Canvas, CanvasConstrain, MaskMemory, apply_mask_alpha_to_pixel, blend_pixel,
    draw_mask, mask_index_from_coord, overlay_area, overlay_image,
  },
  resources::font::ResolvedGlyph,
};

fn invert_y_coordinate(command: Command) -> Command {
  match command {
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
  }
}

pub(crate) fn draw_decoration(
  canvas: &mut Canvas,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  color: Color,
  offset: f32,
  size: f32,
  layout: Layout,
  transform: Affine,
) {
  canvas.fill_color(
    Size {
      width: glyph_run.advance(),
      height: size,
    },
    color,
    BorderProperties::default(),
    transform
      * Affine::translation(
        layout.border.left + layout.padding.left + glyph_run.offset(),
        layout.border.top + layout.padding.top + offset,
      ),
  );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_glyph<I: GenericImageView<Pixel = Rgba<u8>>>(
  glyph: Glyph,
  glyph_content: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  layout: Layout,
  fill_image: Option<&I>,
  mut transform: Affine,
  opacity: u8,
  text_style: &parley::Style<InlineBrush>,
  palette: Option<ColorPalette>,
) -> Result<()> {
  transform *= Affine::translation(
    layout.border.left + layout.padding.left + glyph.x,
    layout.border.top + layout.padding.top + glyph.y,
  );

  match (glyph_content, fill_image) {
    (ResolvedGlyph::Image(bitmap), Some(image_fill)) => {
      transform *= Affine::translation(bitmap.placement.left as f32, -bitmap.placement.top as f32);

      let mask = bitmap
        .data
        .iter()
        .skip(3)
        .step_by(4)
        .copied()
        .collect::<Vec<_>>();

      let mut bottom = RgbaImage::new(bitmap.placement.width, bitmap.placement.height);

      let fill_dimensions = image_fill.dimensions();

      overlay_area(
        &mut bottom,
        Point::ZERO,
        Size {
          width: bitmap.placement.width,
          height: bitmap.placement.height,
        },
        None,
        |x, y| {
          let alpha = mask[mask_index_from_coord(x, y, bitmap.placement.width)];

          let source_x = x + glyph.x as u32;
          let source_y = y + glyph.y as u32 - bitmap.placement.top as u32;

          if source_x >= fill_dimensions.0 || source_y >= fill_dimensions.1 {
            return Color::transparent().into();
          }

          let mut pixel = image_fill.get_pixel(source_x, source_y);

          apply_mask_alpha_to_pixel(&mut pixel, alpha);
          apply_mask_alpha_to_pixel(&mut pixel, opacity);

          pixel
        },
      );

      canvas.overlay_image(
        &bottom,
        BorderProperties::default(),
        transform,
        ImageScalingAlgorithm::Auto,
        255,
      );
    }
    (ResolvedGlyph::Image(bitmap), None) => {
      transform *= Affine::translation(bitmap.placement.left as f32, -bitmap.placement.top as f32);

      let image = RgbaImage::from_raw(
        bitmap.placement.width,
        bitmap.placement.height,
        bitmap.data.clone(),
      )
      .ok_or(ImageError::Decoding(DecodingError::new(
        ImageFormatHint::Unknown,
        "Failed to create image from raw data",
      )))?;

      canvas.overlay_image(
        &image,
        Default::default(),
        transform,
        Default::default(),
        opacity,
      );
    }
    (ResolvedGlyph::Outline(outline), Some(fill_image)) => {
      // If the transform is not invertible, we can't draw the glyph
      let Some(inverse) = transform.invert() else {
        return Ok(());
      };

      let paths = collect_outline_paths(outline);

      maybe_draw_text_shadow(canvas, style, transform, &paths);

      let (mask, placement) = canvas.mask_memory.render(&paths, Some(transform), None);

      overlay_area(
        &mut canvas.image,
        Point {
          x: placement.left as f32,
          y: placement.top as f32,
        },
        Size {
          width: placement.width,
          height: placement.height,
        },
        canvas.constrains.last(),
        |x, y| {
          let alpha = mask[mask_index_from_coord(x, y, placement.width)];

          if alpha == 0 {
            return Color::transparent().into();
          }

          let point = inverse.transform_point(Point {
            x: (x as f32 + placement.left as f32).round(),
            y: (y as f32 + placement.top as f32).round(),
          });

          let point = point
            + Point {
              x: glyph.x,
              y: glyph.y,
            };

          let sampled_pixel = match style.parent.image_rendering {
            ImageScalingAlgorithm::Pixelated => interpolate_nearest(fill_image, point.x, point.y),
            _ => interpolate_bilinear(fill_image, point.x, point.y),
          };

          let Some(mut pixel) = sampled_pixel else {
            return Color::transparent().into();
          };

          blend_pixel(&mut pixel, text_style.brush.color.into());
          apply_mask_alpha_to_pixel(&mut pixel, alpha);
          apply_mask_alpha_to_pixel(&mut pixel, opacity);

          pixel
        },
      );

      maybe_draw_text_stroke(canvas, style, transform, &paths);
    }
    (ResolvedGlyph::Outline(outline), None) => {
      let paths = collect_outline_paths(outline);

      maybe_draw_text_shadow(canvas, style, transform, &paths);

      if outline.is_color()
        && let Some(palette) = palette
      {
        draw_color_outline_image(
          &mut canvas.image,
          &mut canvas.mask_memory,
          outline,
          palette,
          transform,
          opacity,
          canvas.constrains.last(),
        );
      } else {
        let (mask, placement) = canvas.mask_memory.render(&paths, Some(transform), None);

        draw_mask(
          &mut canvas.image,
          mask,
          placement,
          text_style.brush.color,
          canvas.constrains.last(),
        );
      }

      maybe_draw_text_stroke(canvas, style, transform, &paths);
    }
  }

  Ok(())
}

fn maybe_draw_text_stroke(
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
  paths: &[Command],
) {
  if style.stroke_width <= 0.0 {
    return;
  }

  let mut stroke = Stroke::new(style.stroke_width);
  stroke.scale = false;
  stroke.join = Join::Bevel;

  let (stroke_mask, stroke_placement) =
    canvas
      .mask_memory
      .render(paths, Some(transform), Some(stroke.into()));

  draw_mask(
    &mut canvas.image,
    stroke_mask,
    stroke_placement,
    style.text_stroke_color,
    canvas.constrains.last(),
  );
}

fn maybe_draw_text_shadow(
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
  paths: &[Command],
) {
  let Some(ref shadows) = style.text_shadow else {
    return;
  };

  for shadow in shadows.iter() {
    shadow.draw_outset(
      &mut canvas.image,
      &mut canvas.mask_memory,
      canvas.constrains.last(),
      paths,
      transform,
      Default::default(),
    );
  }
}

fn collect_outline_paths(outline: &Outline) -> Vec<Command> {
  outline
    .path()
    .commands()
    .map(invert_y_coordinate)
    .collect::<Vec<_>>()
}

// https://github.com/dfrg/swash/blob/3d8e6a781c93454dadf97e5c15764ceafab228e0/src/scale/mod.rs#L921
#[allow(clippy::too_many_arguments)]
fn draw_color_outline_image(
  canvas: &mut RgbaImage,
  mask_memory: &mut MaskMemory,
  outline: &Outline,
  palette: ColorPalette,
  mut transform: Affine,
  opacity: u8,
  constrain: Option<&CanvasConstrain>,
) {
  // Fast path: if the opacity is 255, we can just draw the outline without any blending
  if opacity == 255 {
    for i in 0..outline.len() {
      let Some(layer) = outline.get(i) else {
        break;
      };

      let Some(color) = layer.color_index().map(|index| Color(palette.get(index))) else {
        continue;
      };

      let paths = layer
        .path()
        .commands()
        .map(invert_y_coordinate)
        .collect::<Vec<_>>();

      let (mask, placement) = mask_memory.render(&paths, Some(transform), None);

      draw_mask(canvas, mask, placement, color, constrain);
    }

    return;
  }

  let translation = transform.decompose_translation();

  transform.x = 0.0;
  transform.y = 0.0;

  let paths = outline
    .path()
    .commands()
    .map(invert_y_coordinate)
    .collect::<Vec<_>>();

  let outer_placement = mask_memory.placement(&paths, Some(transform), None);

  let mut image = RgbaImage::new(outer_placement.width, outer_placement.height);

  for i in 0..outline.len() {
    let Some(layer) = outline.get(i) else {
      break;
    };

    let Some(color) = layer.color_index().map(|index| Color(palette.get(index))) else {
      continue;
    };

    let paths = layer
      .path()
      .commands()
      .map(invert_y_coordinate)
      .collect::<Vec<_>>();

    let (mask, mut placement) = mask_memory.render(&paths, Some(transform), None);

    placement.left -= outer_placement.left;
    placement.top -= outer_placement.top;

    draw_mask(&mut image, mask, placement, color, constrain);
  }

  overlay_image(
    canvas,
    &image,
    BorderProperties::default(),
    Affine::translation(
      translation.x + outer_placement.left as f32,
      translation.y + outer_placement.top as f32,
    ),
    Default::default(),
    opacity,
    constrain,
    mask_memory,
  );
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

/// Use binary search to find the minimum width that maintains the same number of lines.
/// Returns `true` if a meaningful adjustment was made.
pub(crate) fn make_balanced_text(
  inline_layout: &mut InlineLayout,
  max_width: f32,
  target_lines: usize,
) -> bool {
  if target_lines <= 1 {
    return false;
  }

  // Binary search between half width and full width
  let mut left = max_width / 2.0;
  let mut right = max_width;

  // Safety limit on iterations to prevent infinite loops
  const MAX_ITERATIONS: u32 = 20;
  let mut iterations = 0;

  while left + 1.0 < right && iterations < MAX_ITERATIONS {
    iterations += 1;
    let mid = (left + right) / 2.0;

    break_lines(inline_layout, mid, None);
    let lines_at_mid = inline_layout.lines().count();

    if lines_at_mid > target_lines {
      // Too narrow, need more width
      left = mid;
    } else {
      // Can fit in target lines, try narrower
      right = mid;
    }
  }

  let balanced_width = right.ceil();

  // No meaningful adjustment if within 1px of max_width
  if (balanced_width - max_width).abs() < 1.0 {
    // Reset to original max_width
    break_lines(inline_layout, max_width, None);
    false
  } else {
    // Apply the balanced width
    break_lines(inline_layout, balanced_width, None);
    true
  }
}

/// Attempts to avoid orphans (single short words on the last line) by adjusting line breaks.
/// Returns `true` if a meaningful adjustment was made.
pub(crate) fn make_pretty_text(inline_layout: &mut InlineLayout, max_width: f32) -> bool {
  // Get the last line width at the current max width (layout should already be broken)
  let Some(last_line_width) = inline_layout
    .lines()
    .last()
    .map(|line| line.runs().map(|run| run.advance()).sum::<f32>())
  else {
    return false;
  };

  // Check if the last line is too short (less than 1/3 of container width)
  if last_line_width >= max_width / 3.0 {
    return false;
  }

  // Get original line count
  let original_lines = inline_layout.lines().count();

  // Only apply if we have more than one line (single line text doesn't need adjustment)
  if original_lines <= 1 {
    return false;
  }

  // Try reflowing with 90% width to redistribute words
  let adjusted_width = max_width * 0.9;
  break_lines(inline_layout, adjusted_width, None);
  let adjusted_lines = inline_layout.lines().count();

  // Use the adjusted width only if it doesn't add too many lines (at most 30% more)
  let max_acceptable_lines = ((original_lines as f32) * 1.3).ceil() as usize;

  if adjusted_lines <= max_acceptable_lines {
    true
  } else {
    // Reset to original max_width
    break_lines(inline_layout, max_width, None);
    false
  }
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
