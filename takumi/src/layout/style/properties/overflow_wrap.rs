use crate::layout::style::{FromCss, declare_enum_from_css_impl, tw::TailwindPropertyParser};

/// Controls how text should be overflowed.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct OverflowWrap(parley::OverflowWrap);

impl TailwindPropertyParser for OverflowWrap {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}

declare_enum_from_css_impl!(
  OverflowWrap,
  "normal" => OverflowWrap(parley::OverflowWrap::Normal),
  "anywhere" => OverflowWrap(parley::OverflowWrap::Anywhere),
  "break-word" => OverflowWrap(parley::OverflowWrap::BreakWord),
);

impl From<OverflowWrap> for parley::OverflowWrap {
  fn from(value: OverflowWrap) -> Self {
    value.0
  }
}
