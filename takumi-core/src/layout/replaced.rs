//! Where replaced content sits inside its content box.
//!
//! `object-fit` picks a size, `object-position` places it, and content larger
//! than the box has to be clipped to it. That is the same arithmetic whether a
//! backend then samples pixels, writes an `<image>`, or emits an XObject.

use crate::{
  context::RenderContext,
  geometry::{Point, Size},
  style::ObjectFit,
};

/// Where and how large replaced content draws.
#[derive(Debug, Clone, Copy)]
pub struct ReplacedPlacement {
  /// The drawn size.
  pub size: Size<f32>,
  /// The top-left, relative to the content box.
  pub offset: Point<f32>,
}

impl ReplacedPlacement {
  /// Whether the content reaches past its box and needs clipping to it. A
  /// percentage past 100% pushes content that fits out of the box, so the test
  /// is on the placed edges, not the size alone. Half a pixel of slack keeps a
  /// `cover` that lands on the box from opening a clip nothing shows through.
  pub fn overflows(&self, content: Size<f32>) -> bool {
    self.offset.x < -0.5
      || self.offset.y < -0.5
      || self.offset.x + self.size.width > content.width + 0.5
      || self.offset.y + self.size.height > content.height + 0.5
  }
}

/// The part of placed content that lands inside its box.
#[derive(Debug, Clone, Copy)]
pub struct ClippedPlacement {
  /// Where the visible part draws, relative to the content box.
  pub origin: Point<f32>,
  /// How much of the content's top-left the box cuts off, in drawn pixels.
  pub crop: Point<f32>,
  /// The visible size.
  pub size: Size<f32>,
}

impl ReplacedPlacement {
  /// Intersects the placed content with the `content` box.
  pub fn clipped(&self, content: Size<f32>) -> ClippedPlacement {
    let origin = Point {
      x: self.offset.x.max(0.0),
      y: self.offset.y.max(0.0),
    };

    ClippedPlacement {
      origin,
      crop: Point {
        x: (-self.offset.x).max(0.0),
        y: (-self.offset.y).max(0.0),
      },
      size: Size {
        width: ((self.offset.x + self.size.width).min(content.width) - origin.x).max(0.0),
        height: ((self.offset.y + self.size.height).min(content.height) - origin.y).max(0.0),
      },
    }
  }
}

/// Sizes `intrinsic` content for a `content` box and places it.
///
/// `fill` and a missing intrinsic size both stretch to the box, which is what
/// CSS asks for when there is no ratio to preserve.
pub fn place_replaced(
  context: &RenderContext,
  content: Size<f32>,
  intrinsic: Size<f32>,
) -> ReplacedPlacement {
  let scale = match context.style.object_fit {
    _ if intrinsic.width <= 0.0 || intrinsic.height <= 0.0 => None,
    ObjectFit::Fill => None,
    ObjectFit::Contain => {
      Some((content.width / intrinsic.width).min(content.height / intrinsic.height))
    }
    ObjectFit::Cover => {
      Some((content.width / intrinsic.width).max(content.height / intrinsic.height))
    }
    ObjectFit::ScaleDown => Some(
      (content.width / intrinsic.width)
        .min(content.height / intrinsic.height)
        .min(1.0),
    ),
    ObjectFit::None => Some(1.0),
  };
  let size = match scale {
    Some(scale) => Size {
      width: intrinsic.width * scale,
      height: intrinsic.height * scale,
    },
    None => content,
  };
  let position = context.style.object_position.0;

  ReplacedPlacement {
    size,
    offset: Point {
      x: position.x.resolve(context, content.width - size.width),
      y: position.y.resolve(context, content.height - size.height),
    },
  }
}

#[cfg(test)]
mod tests {
  use super::ReplacedPlacement;
  use crate::{
    Fonts,
    context::RenderContext,
    geometry::{Point, Size},
    style::{Length, PositionComponent, PositionKeywordX, SizingContext},
    viewport::Viewport,
  };

  fn axis(component: PositionComponent, available: f32) -> f32 {
    let fonts = Fonts::default();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot_with_fallbacks(None))
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((1200, 630)))
          .font_size(16.0)
          .line_height(0.0)
          .build(),
      )
      .build();

    component.resolve(&context, available)
  }

  #[test]
  fn a_keyword_lands_at_its_share_of_the_free_space() {
    assert_eq!(
      axis(PositionComponent::KeywordX(PositionKeywordX::Center), 120.0),
      60.0
    );
  }

  #[test]
  fn a_length_is_not_scaled_by_the_free_space() {
    assert_eq!(
      axis(PositionComponent::Length(Length::Px(12.0)), 120.0),
      12.0
    );
  }

  #[test]
  fn a_percentage_may_reach_past_the_free_space() {
    assert_eq!(
      axis(PositionComponent::Length(Length::Percentage(150.0)), 120.0),
      180.0
    );
  }

  #[test]
  fn content_pushed_past_its_box_asks_to_be_clipped() {
    let placement = ReplacedPlacement {
      size: Size {
        width: 40.0,
        height: 40.0,
      },
      offset: Point { x: 90.0, y: 0.0 },
    };
    let content = Size {
      width: 100.0,
      height: 100.0,
    };

    assert!(placement.overflows(content));
    assert!(
      !ReplacedPlacement {
        offset: Point { x: 30.0, y: 30.0 },
        ..placement
      }
      .overflows(content)
    );
  }
}
