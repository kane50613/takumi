use crate::style::{TextAlign, impl_css_enum};

/// `text-align-last`: how the last line, and a line before a forced break, aligns.
/// <https://drafts.csswg.org/css-text-3/#text-align-last-property>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TextAlignLast {
  /// As `text-align`, except that `justify` aligns to the start.
  #[default]
  Auto,
  /// To the start edge.
  Start,
  /// To the end edge.
  End,
  /// To the left edge.
  Left,
  /// To the right edge.
  Right,
  /// Centered.
  Center,
  /// Justified.
  Justify,
}

impl_css_enum!(
  TextAlignLast,
  "auto" => TextAlignLast::Auto,
  "start" => TextAlignLast::Start,
  "end" => TextAlignLast::End,
  "left" => TextAlignLast::Left,
  "right" => TextAlignLast::Right,
  "center" => TextAlignLast::Center,
  "justify" => TextAlignLast::Justify,
);

impl TextAlignLast {
  /// The alignment of a last line whose block has `text_align`.
  pub(crate) fn resolve(self, text_align: TextAlign) -> TextAlign {
    match self {
      Self::Auto if text_align == TextAlign::Justify => TextAlign::Start,
      Self::Auto => text_align,
      Self::Start => TextAlign::Start,
      Self::End => TextAlign::End,
      Self::Left => TextAlign::Left,
      Self::Right => TextAlign::Right,
      Self::Center => TextAlign::Center,
      Self::Justify => TextAlign::Justify,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::TextAlignLast;
  use crate::style::TextAlign;

  #[test]
  fn auto_aligns_a_justified_last_line_to_the_start() {
    assert_eq!(
      TextAlignLast::Auto.resolve(TextAlign::Justify),
      TextAlign::Start
    );
    assert_eq!(
      TextAlignLast::Auto.resolve(TextAlign::Center),
      TextAlign::Center
    );
    assert_eq!(
      TextAlignLast::Justify.resolve(TextAlign::Left),
      TextAlign::Justify
    );
  }
}
