use crate::style::impl_css_enum;

/// `line-break`: how strictly lines break around CJK punctuation and small kana.
/// <https://drafts.csswg.org/css-text-3/#line-break-property>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum LineBreak {
  /// The UA's default, which takumi and Blink take as ICU's default rules.
  #[default]
  Auto,
  /// The least restrictive rules, for short lines such as newspapers'.
  Loose,
  /// The most common rules.
  Normal,
  /// The most restrictive rules.
  Strict,
  /// A soft wrap opportunity around every typographic character unit.
  Anywhere,
}

impl_css_enum!(
  LineBreak,
  "auto" => LineBreak::Auto,
  "loose" => LineBreak::Loose,
  "normal" => LineBreak::Normal,
  "strict" => LineBreak::Strict,
  "anywhere" => LineBreak::Anywhere,
);
