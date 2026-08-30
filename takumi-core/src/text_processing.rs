//! Backend-agnostic text processing: whitespace collapsing, text-transform, and
//! line balancing/rebreaking used by inline layout.

use std::borrow::Cow;

use parley::layout::BreakReason;

use crate::{
  layout::inline::{InlineLayout, ProcessedInlineSpan, break_lines},
  style::{TextTransform, TextWrapMode, WhiteSpaceCollapse},
};

/// Constraint capping a text block's height.
#[derive(Clone, Copy, Debug)]
pub enum MaxHeight {
  /// Fixed pixel height.
  Absolute(f32),
  /// Maximum line count.
  Lines(u32),
  /// Pixel height and line count, whichever is smaller.
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

/// Expands each tab to `tab_spaces` spaces. Preserved tabs otherwise reach the shaper, which
/// has no space fallback for U+0009 and renders a font-dependent glyph.
fn expand_tabs(input: &str, tab_spaces: usize) -> Cow<'_, str> {
  if !input.contains('\t') {
    return Cow::Borrowed(input);
  }

  let mut out = String::with_capacity(input.len() + tab_spaces);
  for ch in input.chars() {
    if ch == '\t' {
      out.extend(std::iter::repeat_n(' ', tab_spaces));
    } else {
      out.push(ch);
    }
  }

  Cow::Owned(out)
}

/// Applies whitespace collapse rules to the input text according to `WhiteSpaceCollapse`.
pub(crate) fn apply_white_space_collapse<'a>(
  input: &'a str,
  collapse: WhiteSpaceCollapse,
  tab_spaces: usize,
  previous_collapsible_space: &mut bool,
  previous_was_line_break: &mut bool,
) -> Cow<'a, str> {
  // An empty span contributes no characters, so boundary state carries through.
  if input.is_empty() {
    return Cow::Borrowed(input);
  }

  match collapse {
    WhiteSpaceCollapse::Preserve => {
      let expanded = expand_tabs(input, tab_spaces);

      // A following collapsible span drops its leading space when this span
      // already ends in whitespace, so carry that state across the mode switch.
      // A span expanded to nothing (all tabs, tab-size 0) carries state through.
      if let Some(last) = expanded.chars().next_back() {
        *previous_collapsible_space = last.is_whitespace();
        *previous_was_line_break = false;
      }
      expanded
    }

    // Collapse sequences of whitespace (spaces, tabs, line breaks) into a single space
    // and trim leading/trailing spaces.
    WhiteSpaceCollapse::Collapse => {
      let mut out = String::with_capacity(input.len());
      let mut last_was_ws = *previous_collapsible_space;

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

      *previous_collapsible_space = last_was_ws;
      *previous_was_line_break = false;
      Cow::Owned(out)
    }

    // Preserve sequences of spaces/tabs but remove line breaks (replace them with a single space).
    WhiteSpaceCollapse::PreserveSpaces => {
      let mut out = String::with_capacity(input.len());
      let mut last_was_space = *previous_collapsible_space;

      for ch in input.chars() {
        // treat common line break characters as breaks to be removed/replaced
        if matches!(ch, '\n' | '\r' | '\x0B' | '\x0C' | '\u{2028}' | '\u{2029}') {
          if !last_was_space {
            out.push(' ');
            last_was_space = true;
          }
        } else if ch == '\t' {
          out.extend(std::iter::repeat_n(' ', tab_spaces));
          if tab_spaces > 0 {
            last_was_space = true;
          }
        } else {
          out.push(ch);
          last_was_space = ch == ' ';
        }
      }

      *previous_collapsible_space = last_was_space;
      *previous_was_line_break = false;
      Cow::Owned(out)
    }

    // Preserve line breaks but collapse consecutive spaces and tabs into single spaces.
    // Also remove leading spaces after line breaks.
    WhiteSpaceCollapse::PreserveBreaks => {
      let mut out = String::with_capacity(input.len());
      let mut last_was_space = *previous_collapsible_space;
      let mut last_was_line_break = *previous_was_line_break;

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

      *previous_collapsible_space = last_was_space;
      *previous_was_line_break = last_was_line_break;
      Cow::Owned(out)
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

#[derive(Clone, Copy)]
pub(crate) struct RebreakOptions {
  pub(crate) max_width: f32,
  pub(crate) max_height: Option<MaxHeight>,
  pub(crate) line_height_hint: f32,
  pub(crate) text_wrap_mode: TextWrapMode,
}

/// Use binary search to find the minimum width that maintains the same number of lines.
/// Returns `true` if a meaningful adjustment was made.
pub(crate) fn make_balanced_text(
  inline_layout: &mut InlineLayout,
  options: RebreakOptions,
  target_lines: usize,
  device_pixel_ratio: f32,
  spans: &[ProcessedInlineSpan<'_>],
  positioned_floats: &mut Vec<parley::PositionedInlineBox>,
) -> bool {
  let RebreakOptions {
    max_width,
    max_height,
    line_height_hint,
    text_wrap_mode,
  } = options;
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

    positioned_floats.clear();
    break_lines(
      inline_layout,
      mid,
      None,
      line_height_hint,
      text_wrap_mode,
      spans,
      positioned_floats,
    );
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
    positioned_floats.clear();
    break_lines(
      inline_layout,
      max_width,
      max_height,
      line_height_hint,
      text_wrap_mode,
      spans,
      positioned_floats,
    );
    false
  } else {
    // Apply the balanced width
    positioned_floats.clear();
    break_lines(
      inline_layout,
      balanced_width,
      max_height,
      line_height_hint,
      text_wrap_mode,
      spans,
      positioned_floats,
    );
    true
  }
}

/// Attempts to avoid orphans (single short words on the last line) by adjusting line breaks.
/// Returns `true` if a meaningful adjustment was made.
pub(crate) fn make_pretty_text(
  inline_layout: &mut InlineLayout,
  options: RebreakOptions,
  spans: &[ProcessedInlineSpan<'_>],
  positioned_floats: &mut Vec<parley::PositionedInlineBox>,
) -> bool {
  let RebreakOptions {
    max_width,
    max_height,
    line_height_hint,
    text_wrap_mode,
  } = options;
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
  positioned_floats.clear();
  break_lines(
    inline_layout,
    adjusted_width,
    max_height,
    line_height_hint,
    text_wrap_mode,
    spans,
    positioned_floats,
  );
  let adjusted_lines = inline_layout.lines().count();

  // Use the adjusted width only if it doesn't add too many lines (at most 30% more)
  let max_acceptable_lines = ((original_lines as f32) * 1.3).ceil() as usize;

  if adjusted_lines <= max_acceptable_lines {
    true
  } else {
    // Reset to original max_width
    positioned_floats.clear();
    break_lines(
      inline_layout,
      max_width,
      max_height,
      line_height_hint,
      text_wrap_mode,
      spans,
      positioned_floats,
    );
    false
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_white_space_preserve() {
    let input = "  a  b\n";
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;
    let out = apply_white_space_collapse(
      input,
      WhiteSpaceCollapse::Preserve,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    assert_eq!(out, input);
  }

  #[test]
  fn test_white_space_preserve_expands_tabs() {
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;
    let out = apply_white_space_collapse(
      "\ta\tb",
      WhiteSpaceCollapse::Preserve,
      2,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    assert_eq!(out, "  a  b");
    assert!(!previous_collapsible_space);
  }

  #[test]
  fn test_white_space_preserve_tab_size_zero_carries_state() {
    let mut previous_collapsible_space = true;
    let mut previous_was_line_break = false;
    let out = apply_white_space_collapse(
      "\t",
      WhiteSpaceCollapse::Preserve,
      0,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    assert_eq!(out, "");
    assert!(previous_collapsible_space);
  }

  #[test]
  fn test_white_space_preserve_spaces_expands_tabs() {
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;
    let out = apply_white_space_collapse(
      "a\tb\nc",
      WhiteSpaceCollapse::PreserveSpaces,
      4,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    assert_eq!(out, "a    b c");
  }

  #[test]
  fn test_white_space_collapse() {
    let input = "  a \n\t b  c\n\n ";
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;
    let out = apply_white_space_collapse(
      input,
      WhiteSpaceCollapse::Collapse,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    assert_eq!(out, " a b c ");
  }

  #[test]
  fn test_white_space_preserve_spaces() {
    let input = "a \n b";
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;
    let out = apply_white_space_collapse(
      input,
      WhiteSpaceCollapse::PreserveSpaces,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    // line break should be replaced with a single space; existing spaces preserved
    assert_eq!(out, "a  b");
  }

  #[test]
  fn test_white_space_preserve_breaks() {
    let input = "a \n b\tc";
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;
    let out = apply_white_space_collapse(
      input,
      WhiteSpaceCollapse::PreserveBreaks,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    // spaces and tabs collapsed to single space, line break preserved
    assert_eq!(out, "a \nb c");
  }

  #[test]
  fn test_white_space_collapse_preserves_boundary_space_across_spans() {
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;
    let left = apply_white_space_collapse(
      "A",
      WhiteSpaceCollapse::Collapse,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    let middle = apply_white_space_collapse(
      " ",
      WhiteSpaceCollapse::Collapse,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    let right = apply_white_space_collapse(
      "B",
      WhiteSpaceCollapse::Collapse,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );

    assert_eq!(format!("{left}{middle}{right}"), "A B");
  }

  #[test]
  fn test_white_space_collapse_merges_adjacent_span_spaces() {
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;
    let left = apply_white_space_collapse(
      "A ",
      WhiteSpaceCollapse::Collapse,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    let right = apply_white_space_collapse(
      " B",
      WhiteSpaceCollapse::Collapse,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );

    assert_eq!(format!("{left}{right}"), "A B");
  }

  #[test]
  fn test_white_space_collapse_after_preserve_span_drops_leading_space() {
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;
    let left = apply_white_space_collapse(
      "A ",
      WhiteSpaceCollapse::Preserve,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    let right = apply_white_space_collapse(
      " B",
      WhiteSpaceCollapse::Collapse,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );

    assert_eq!(format!("{left}{right}"), "A B");
  }

  #[test]
  fn test_white_space_empty_span_keeps_boundary_state() {
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;
    let left = apply_white_space_collapse(
      "A ",
      WhiteSpaceCollapse::Collapse,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    let middle = apply_white_space_collapse(
      "",
      WhiteSpaceCollapse::Preserve,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    let right = apply_white_space_collapse(
      " B",
      WhiteSpaceCollapse::Collapse,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );

    assert_eq!(format!("{left}{middle}{right}"), "A B");
  }

  #[test]
  fn test_white_space_preserve_breaks_strips_spaces_after_span_boundary_line_break() {
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;
    let left = apply_white_space_collapse(
      "A\n",
      WhiteSpaceCollapse::PreserveBreaks,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );
    let right = apply_white_space_collapse(
      "   B",
      WhiteSpaceCollapse::PreserveBreaks,
      8,
      &mut previous_collapsible_space,
      &mut previous_was_line_break,
    );

    assert_eq!(format!("{left}{right}"), "A\nB");
  }
}
