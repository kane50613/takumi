//! Blink's [`FontBaseline`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_baseline.h).

/// Blink's `FontBaseline`: a baseline of the CSS Inline Layout module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FontBaseline {
  Alphabetic,
  Central,
  TextUnder,
  IdeographicUnder,
  XMiddle,
  Math,
  Hanging,
  TextOver,
}
