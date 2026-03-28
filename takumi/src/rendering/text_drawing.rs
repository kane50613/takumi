use std::{borrow::Cow, convert::Into};

use image::{GenericImageView, Rgba, RgbaImage};
use parley::{GlyphRun, layout::BreakReason};
use skrifa::color::ColorPalette;
use taffy::{Layout, Point, Size};
use zeno::{Command, Stroke};

use crate::{
  Result,
  layout::{
    inline::{InlineBrush, InlineLayout, break_lines},
    style::{
      Affine, BlendMode, Color, ImageScalingAlgorithm, SizedFontStyle, TextTransform,
      WhiteSpaceCollapse,
    },
  },
  rendering::{
    BorderProperties, BufferPool, Canvas, CanvasConstrain, ColorTile, MaskMemory,
    apply_mask_alpha_to_pixel, blend_pixel, draw_mask, mask_index_from_coord, overlay_area,
    sample_transformed_pixel,
  },
  resources::font::{ResolvedColorLayer, ResolvedGlyph},
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
  let start_x = layout.border.left + layout.padding.left + glyph_run.offset();
  let end_x = start_x + glyph_run.advance();
  if end_x <= start_x {
    return;
  }

  let snapped_start_x = start_x.floor();
  let width = (end_x.ceil() - snapped_start_x) as u32;

  let tile = ColorTile {
    color: color.into(),
    width,
    height: size as u32,
  };

  canvas.overlay_image(
    &tile,
    BorderProperties::default(),
    transform
      * Affine::translation(
        snapped_start_x,
        layout.border.top + layout.padding.top + offset,
      ),
    ImageScalingAlgorithm::Auto,
    BlendMode::Normal,
  );
}

pub(crate) fn draw_glyph_clip_image<I: GenericImageView<Pixel = Rgba<u8>>>(
  glyph: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  mut transform: Affine,
  inline_offset: Point<f32>,
  clip_image: &I,
) -> Result<()> {
  transform *= Affine::translation(inline_offset.x, inline_offset.y);

  match glyph {
    ResolvedGlyph::Bitmap(bitmap) => {
      transform *= Affine::translation(bitmap.placement.left as f32, -bitmap.placement.top as f32);

      let mask_capacity = (bitmap.placement.width * bitmap.placement.height) as usize;
      let mut mask = canvas.buffer_pool.acquire_dirty(mask_capacity);
      for (i, alpha) in bitmap
        .image
        .as_raw()
        .iter()
        .skip(3)
        .step_by(4)
        .copied()
        .enumerate()
      {
        if i < mask.len() {
          mask[i] = alpha;
        }
      }

      let mut bottom = canvas
        .buffer_pool
        .acquire_image(bitmap.placement.width, bitmap.placement.height)?;

      let fill_dimensions = clip_image.dimensions();

      overlay_area(
        &mut bottom,
        Point::ZERO,
        Size {
          width: bitmap.placement.width,
          height: bitmap.placement.height,
        },
        BlendMode::Normal,
        &[],
        |x, y| {
          let alpha = mask[mask_index_from_coord(x, y, bitmap.placement.width)];

          let source_x = (x as i32 + inline_offset.x as i32 + bitmap.placement.left) as u32;
          let source_y = (y as i32 + inline_offset.y as i32 - bitmap.placement.top) as u32;

          if source_x >= fill_dimensions.0 || source_y >= fill_dimensions.1 {
            return Color::transparent().into();
          }

          let mut pixel = clip_image.get_pixel(source_x, source_y);

          apply_mask_alpha_to_pixel(&mut pixel, alpha);

          pixel
        },
      );

      canvas.overlay_image(
        &bottom,
        BorderProperties::default(),
        transform,
        ImageScalingAlgorithm::Auto,
        BlendMode::Normal,
      );

      canvas.buffer_pool.release_image(bottom);
      canvas.buffer_pool.release(mask);
    }
    ResolvedGlyph::Outline(outline) => {
      // If the transform is not invertible, we can't draw the glyph
      let Some(inverse) = transform.invert() else {
        return Ok(());
      };

      let (mask, placement) = canvas.mask_memory.render(
        &outline.paths,
        Some(transform),
        None,
        &mut canvas.buffer_pool,
      );

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
        BlendMode::Normal,
        &canvas.constrains,
        |x, y| {
          let alpha = mask[mask_index_from_coord(x, y, placement.width)];

          if alpha == 0 {
            return Color::transparent().into();
          }

          let sampled_pixel = sample_transformed_pixel(
            clip_image,
            inverse,
            style.parent.image_rendering,
            (x as i32 + placement.left) as f32,
            (y as i32 + placement.top) as f32,
            inline_offset,
          );

          let Some(mut pixel) = sampled_pixel else {
            return Color::transparent().into();
          };

          apply_mask_alpha_to_pixel(&mut pixel, alpha);

          pixel
        },
      );

      canvas.buffer_pool.release(mask);

      if let Some(embolden) = outline.embolden {
        draw_text_embolden_clip_image(
          canvas,
          style,
          transform,
          &outline.paths,
          embolden,
          clip_image,
          inline_offset,
        );
      }

      draw_text_stroke_clip_image(
        canvas,
        style,
        transform,
        &outline.paths,
        clip_image,
        inline_offset,
      );
    }
  }

  Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_glyph(
  glyph: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  mut transform: Affine,
  inline_offset: Point<f32>,
  color: Color,
  palette: Option<&ColorPalette>,
) -> Result<()> {
  transform *= Affine::translation(inline_offset.x, inline_offset.y);

  match glyph {
    ResolvedGlyph::Bitmap(bitmap) => {
      transform *= Affine::translation(bitmap.placement.left as f32, -bitmap.placement.top as f32);

      canvas.overlay_image(
        &bitmap.image,
        Default::default(),
        transform,
        Default::default(),
        BlendMode::Normal,
      );
    }
    ResolvedGlyph::Outline(outline) => {
      if let Some(color_layers) = outline.color_layers.as_ref()
        && let Some(palette) = palette
      {
        draw_color_outline_image(
          &mut canvas.image,
          &mut canvas.mask_memory,
          &mut canvas.buffer_pool,
          color_layers,
          palette,
          transform,
          &canvas.constrains,
          color.0[3],
        );
      } else {
        let (mask, placement) = canvas.mask_memory.render(
          &outline.paths,
          Some(transform),
          None,
          &mut canvas.buffer_pool,
        );

        draw_mask(
          &mut canvas.image,
          &mask,
          placement,
          color,
          BlendMode::Normal,
          &canvas.constrains,
        );

        canvas.buffer_pool.release(mask);
      }

      if let Some(embolden) = outline.embolden {
        draw_text_embolden(canvas, transform, &outline.paths, color, embolden);
      }

      draw_text_stroke(canvas, style, transform, &outline.paths);
    }
  }

  Ok(())
}

fn draw_text_stroke_clip_image<I: GenericImageView<Pixel = Rgba<u8>>>(
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
  paths: &[Command],
  clip_image: &I,
  inline_offset: Point<f32>,
) {
  if style.stroke_width <= 0.0 {
    return;
  }

  let Some(inverse) = transform.invert() else {
    return;
  };

  let mut stroke = Stroke::new(style.stroke_width);
  stroke.join = style.parent.stroke_linejoin.into();

  let (stroke_mask, stroke_placement) = canvas.mask_memory.render(
    paths,
    Some(transform),
    Some(stroke.into()),
    &mut canvas.buffer_pool,
  );

  overlay_area(
    &mut canvas.image,
    Point {
      x: stroke_placement.left as f32,
      y: stroke_placement.top as f32,
    },
    Size {
      width: stroke_placement.width,
      height: stroke_placement.height,
    },
    BlendMode::Normal,
    &canvas.constrains,
    |x, y| {
      let alpha = stroke_mask[mask_index_from_coord(x, y, stroke_placement.width)];

      if alpha == 0 {
        return Color::transparent().into();
      }

      let inline_x = (x as i32 + stroke_placement.left) as f32;
      let inline_y = (y as i32 + stroke_placement.top) as f32;

      let sampled_pixel = sample_transformed_pixel(
        clip_image,
        inverse,
        style.parent.image_rendering,
        inline_x,
        inline_y,
        inline_offset,
      );

      let Some(mut pixel) = sampled_pixel else {
        return Color::transparent().into();
      };

      blend_pixel(
        &mut pixel,
        style.text_stroke_color.into(),
        BlendMode::Normal,
      );
      apply_mask_alpha_to_pixel(&mut pixel, alpha);

      pixel
    },
  );

  canvas.buffer_pool.release(stroke_mask);
}

fn draw_text_embolden_clip_image<I: GenericImageView<Pixel = Rgba<u8>>>(
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
  paths: &[Command],
  embolden: f32,
  clip_image: &I,
  inline_offset: Point<f32>,
) {
  if embolden <= 0.0 {
    return;
  }

  let Some(inverse) = transform.invert() else {
    return;
  };

  let mut stroke = Stroke::new(embolden * 2.0);
  stroke.join = style.parent.stroke_linejoin.into();

  let (stroke_mask, stroke_placement) = canvas.mask_memory.render(
    paths,
    Some(transform),
    Some(stroke.into()),
    &mut canvas.buffer_pool,
  );

  overlay_area(
    &mut canvas.image,
    Point {
      x: stroke_placement.left as f32,
      y: stroke_placement.top as f32,
    },
    Size {
      width: stroke_placement.width,
      height: stroke_placement.height,
    },
    BlendMode::Normal,
    &canvas.constrains,
    |x, y| {
      let alpha = stroke_mask[mask_index_from_coord(x, y, stroke_placement.width)];

      if alpha == 0 {
        return Color::transparent().into();
      }

      let inline_x = (x as i32 + stroke_placement.left) as f32;
      let inline_y = (y as i32 + stroke_placement.top) as f32;

      let sampled_pixel = sample_transformed_pixel(
        clip_image,
        inverse,
        style.parent.image_rendering,
        inline_x,
        inline_y,
        inline_offset,
      );

      let Some(mut pixel) = sampled_pixel else {
        return Color::transparent().into();
      };

      apply_mask_alpha_to_pixel(&mut pixel, alpha);
      pixel
    },
  );

  canvas.buffer_pool.release(stroke_mask);
}

fn draw_text_stroke(
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
  paths: &[Command],
) {
  if style.stroke_width <= 0.0 {
    return;
  }

  let mut stroke = Stroke::new(style.stroke_width);
  stroke.join = style.parent.stroke_linejoin.into();

  let (stroke_mask, stroke_placement) = canvas.mask_memory.render(
    paths,
    Some(transform),
    Some(stroke.into()),
    &mut canvas.buffer_pool,
  );

  draw_mask(
    &mut canvas.image,
    &stroke_mask,
    stroke_placement,
    style.text_stroke_color,
    BlendMode::Normal,
    &canvas.constrains,
  );

  canvas.buffer_pool.release(stroke_mask);
}

fn draw_text_embolden(
  canvas: &mut Canvas,
  transform: Affine,
  paths: &[Command],
  color: Color,
  embolden: f32,
) {
  if embolden <= 0.0 {
    return;
  }

  let stroke = Stroke::new(embolden * 2.0);

  let (stroke_mask, stroke_placement) = canvas.mask_memory.render(
    paths,
    Some(transform),
    Some(stroke.into()),
    &mut canvas.buffer_pool,
  );

  draw_mask(
    &mut canvas.image,
    &stroke_mask,
    stroke_placement,
    color,
    BlendMode::Normal,
    &canvas.constrains,
  );

  canvas.buffer_pool.release(stroke_mask);
}

fn draw_text_shadow(
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
  paths: &[Command],
) -> Result<()> {
  if style.text_shadow.is_empty() {
    return Ok(());
  }

  for shadow in style.text_shadow.iter() {
    shadow.draw_outset(canvas, paths, transform, Default::default(), None)?;
  }

  Ok(())
}

pub(crate) fn draw_glyph_text_shadow(
  glyph: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  mut transform: Affine,
  inline_offset: Point<f32>,
) -> Result<()> {
  transform *= Affine::translation(inline_offset.x, inline_offset.y);

  if let ResolvedGlyph::Outline(outline) = glyph {
    draw_text_shadow(canvas, style, transform, &outline.paths)?;
  }

  Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_color_outline_image(
  canvas: &mut RgbaImage,
  mask_memory: &mut MaskMemory,
  buffer_pool: &mut BufferPool,
  color_layers: &[ResolvedColorLayer],
  palette: &ColorPalette,
  transform: Affine,
  constrains: &[CanvasConstrain],
  opacity: u8,
) {
  if opacity == 0 {
    return;
  }

  for layer in color_layers {
    let Some(record) = palette.colors().get(usize::from(layer.palette_index)) else {
      continue;
    };
    let alpha = ((record.alpha() as f32 / 255.0) * layer.alpha * (opacity as f32 / 255.0) * 255.0)
      .round()
      .clamp(0.0, 255.0) as u8;
    let color = Color([record.red(), record.green(), record.blue(), alpha]);

    let (mask, placement) = mask_memory.render(&layer.paths, Some(transform), None, buffer_pool);

    draw_mask(
      canvas,
      &mask,
      placement,
      color,
      BlendMode::Normal,
      constrains,
    );
    buffer_pool.release(mask);
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

// Preserve the original number of forced breaks while balancing so #437 does not
// reintroduce mid-word splits under `word-break: break-word`.
fn count_emergency_line_breaks(layout: &InlineLayout) -> usize {
  let line_count = layout.lines().count();

  layout
    .lines()
    .take(line_count.saturating_sub(1))
    .filter(|line| line.break_reason() == BreakReason::Emergency)
    .count()
}

/// Use binary search to find the minimum width that maintains the same number of lines.
/// Returns `true` if a meaningful adjustment was made.
pub(crate) fn make_balanced_text(
  inline_layout: &mut InlineLayout,
  max_width: f32,
  max_height: Option<MaxHeight>,
  target_lines: usize,
  device_pixel_ratio: f32,
) -> bool {
  if target_lines <= 1 {
    return false;
  }

  let initial_emergency_breaks = count_emergency_line_breaks(inline_layout);

  // Binary search between half width and full width
  let mut left = max_width / 2.0;
  let mut right = max_width;

  // Safety limit on iterations to prevent infinite loops
  const MAX_ITERATIONS: u32 = 20;
  let mut iterations = 0;

  while left + device_pixel_ratio < right && iterations < MAX_ITERATIONS {
    iterations += 1;
    let mid = (left + right) / 2.0;

    break_lines(inline_layout, mid, None);
    let lines_at_mid = inline_layout.lines().count();

    if lines_at_mid > target_lines
      || count_emergency_line_breaks(inline_layout) > initial_emergency_breaks
    {
      left = mid;
    } else {
      // Can fit in target lines, try narrower
      right = mid;
    }
  }

  let balanced_width = right.ceil();

  // No meaningful adjustment if within 1px * DPR of max_width
  if (balanced_width - max_width).abs() < device_pixel_ratio {
    // Reset to original max_width
    break_lines(inline_layout, max_width, max_height);
    false
  } else {
    // Apply the balanced width
    break_lines(inline_layout, balanced_width, max_height);
    true
  }
}

/// Attempts to avoid orphans (single short words on the last line) by adjusting line breaks.
/// Returns `true` if a meaningful adjustment was made.
pub(crate) fn make_pretty_text(
  inline_layout: &mut InlineLayout,
  max_width: f32,
  max_height: Option<MaxHeight>,
) -> bool {
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
    break_lines(inline_layout, max_width, max_height);
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
