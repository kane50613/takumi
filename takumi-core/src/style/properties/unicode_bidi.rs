use crate::style::impl_css_enum;

/// `unicode-bidi`: how a box's text joins the bidi algorithm, with its `direction`.
/// <https://drafts.csswg.org/css-writing-modes-3/#unicode-bidi>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum UnicodeBidi {
  /// No embedding level of its own.
  #[default]
  Normal,
  /// An embedding level in its `direction`.
  Embed,
  /// An isolated run in its `direction`.
  Isolate,
  /// Its characters all run in its `direction`.
  BidiOverride,
  /// An isolated run whose characters all run in its `direction`.
  IsolateOverride,
  /// Paragraphs whose direction their first strong character decides.
  Plaintext,
}

impl_css_enum!(
  UnicodeBidi,
  "normal" => UnicodeBidi::Normal,
  "embed" => UnicodeBidi::Embed,
  "isolate" => UnicodeBidi::Isolate,
  "bidi-override" => UnicodeBidi::BidiOverride,
  "isolate-override" => UnicodeBidi::IsolateOverride,
  "plaintext" => UnicodeBidi::Plaintext,
);
