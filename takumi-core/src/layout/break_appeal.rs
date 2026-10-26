//! Blink's [`BreakAppeal`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/break_appeal.h).

/// How good a breakpoint is, by the worst rule breaking there violates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BreakAppeal {
  /// Not a class A, B, or C breakpoint of css-break-3 §4.1; it only keeps monolithic content
  /// whole.
  #[default]
  LastResort,
  /// Violates an `avoid` value of `break-before`, `break-after`, or `break-inside`.
  ViolatingBreakAvoid,
  /// Violates `orphans` or `widows` alone.
  ViolatingOrphansAndWidows,
  /// Violates nothing, as every forced break.
  Perfect,
}
