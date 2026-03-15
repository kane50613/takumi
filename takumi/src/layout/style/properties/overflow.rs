use cssparser::match_ignore_ascii_case;
use taffy::Overflow as TaffyOverflow;

use crate::layout::style::{declare_enum_from_css_impl, tw::TailwindPropertyParser};

/// How children overflowing their container should affect layout
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum Overflow {
  /// The automatic minimum size of this node as a flexbox/grid item should be based on the size of its content.
  /// Content that overflows this node *should* contribute to the scroll region of its parent.
  #[default]
  Visible,
  /// The automatic minimum size of this node as a flexbox/grid item should be based on the size of its content.
  /// Content that overflows this node should *not* contribute to the scroll region of its parent.
  Clip,
  /// The automatic minimum size of this node as a flexbox/grid item should be `0`.
  /// Content that overflows this node should *not* contribute to the scroll region of its parent.
  Hidden,
}

declare_enum_from_css_impl!(
  Overflow,
  "visible" => Overflow::Visible,
  "clip" => Overflow::Clip,
  "hidden" => Overflow::Hidden,
);

impl TailwindPropertyParser for Overflow {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "visible" => Some(Overflow::Visible),
      "clip" => Some(Overflow::Clip),
      "hidden" => Some(Overflow::Hidden),
      _ => None,
    }
  }
}

impl From<Overflow> for TaffyOverflow {
  fn from(val: Overflow) -> Self {
    match val {
      Overflow::Visible => TaffyOverflow::Visible,
      Overflow::Clip => TaffyOverflow::Clip,
      Overflow::Hidden => TaffyOverflow::Hidden,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::style::FromCss;

  #[test]
  fn parses_css_clip() {
    assert_eq!(Overflow::from_str("clip"), Ok(Overflow::Clip));
  }

  #[test]
  fn parses_tailwind_clip() {
    assert_eq!(Overflow::parse_tw("clip"), Some(Overflow::Clip));
  }
}
