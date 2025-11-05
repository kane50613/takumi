use cssparser::{Parser, match_ignore_ascii_case};
use serde::{Deserialize, Serialize};
use taffy::{Layout, Size};
use ts_rs::TS;

use crate::{
  layout::{
    Viewport,
    style::{FromCss, ParseResult, SpacePair, tw::TailwindPropertyParser},
  },
  rendering::Canvas,
};

/// How children overflowing their container should affect layout
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Overflow {
  /// The automatic minimum size of this node as a flexbox/grid item should be based on the size of its content.
  /// Content that overflows this node *should* contribute to the scroll region of its parent.
  #[default]
  Visible,
  /// The automatic minimum size of this node as a flexbox/grid item should be `0`.
  /// Content that overflows this node should *not* contribute to the scroll region of its parent.
  Hidden,
}

impl TailwindPropertyParser for Overflow {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "visible" => Some(Overflow::Visible),
      "hidden" => Some(Overflow::Hidden),
      _ => None,
    }
  }
}

impl From<Overflow> for taffy::Overflow {
  fn from(val: Overflow) -> Self {
    match val {
      Overflow::Visible => taffy::Overflow::Visible,
      Overflow::Hidden => taffy::Overflow::Hidden,
    }
  }
}

impl<'i> FromCss<'i> for Overflow {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;

    match_ignore_ascii_case! { ident,
      "visible" => Ok(Overflow::Visible),
      "hidden" => Ok(Overflow::Hidden),
      _ => Err(location.new_unexpected_token_error(
        cssparser::Token::Ident(ident.clone())
      )),
    }
  }
}

/// Represents overflow values for both axes.
///
/// Can be either a single value applied to both axes, or separate values
/// for horizontal and vertical overflow.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
pub struct Overflows(pub SpacePair<Overflow>);

impl Default for Overflows {
  fn default() -> Self {
    Self(SpacePair::from_single(Overflow::Visible))
  }
}

impl Overflows {
  #[inline]
  pub(crate) fn should_clip_content(&self) -> bool {
    *self != Overflows(SpacePair::from_single(Overflow::Visible))
  }

  pub(crate) fn create_clip_canvas(&self, viewport: Viewport, layout: Layout) -> Option<Canvas> {
    let inner_size = Size {
      width: if self.0.x == Overflow::Visible {
        viewport.width
      } else {
        (layout.size.width - layout.padding.right - layout.border.right) as u32
      },
      height: if self.0.y == Overflow::Visible {
        viewport.height
      } else {
        (layout.size.height - layout.padding.bottom - layout.border.bottom) as u32
      },
    };

    if inner_size.width == 0 || inner_size.height == 0 {
      return None;
    }

    Some(Canvas::new(inner_size))
  }
}
