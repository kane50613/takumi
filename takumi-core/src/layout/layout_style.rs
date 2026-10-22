//! The style a box lays out with: its computed box model, with lengths in device px.

use taffy::{Clear, CompactLength, Dimension, Float, LengthPercentage, LengthPercentageAuto};

use crate::{
  geometry::{Point, Rect, Size},
  layout_unit::LayoutUnit,
  style::{
    AlignItems, BoxSizing, ComputedStyle, Contain, Direction, Display, Length, MaxSize, Overflow,
    Position, Size as SizeValue, SizingContext, calc::CalcLinear,
  },
};

/// A box-model length, its absolute and font-relative units resolved to device px.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum LayoutLength {
  #[default]
  Auto,
  Fixed(f32),
  /// A fraction of the percentage basis.
  Percent(f32),
  /// `px` plus a fraction of the percentage basis.
  Calc {
    px: f32,
    percent: f32,
  },
  MinContent,
  MaxContent,
  FitContent,
  Stretch,
  /// Blink's `MinIntrinsic`: a box's automatic minimum size, which no style names.
  MinIntrinsic,
}

impl LayoutLength {
  /// A `width` or `height`.
  pub(crate) fn from_size(size: SizeValue, sizing: &SizingContext) -> Self {
    match size {
      SizeValue::Length(length) => length.to_layout_length(sizing),
      SizeValue::MinContent => Self::MinContent,
      SizeValue::MaxContent => Self::MaxContent,
      SizeValue::FitContent => Self::FitContent,
      SizeValue::Stretch => Self::Stretch,
    }
  }

  /// A `max-width` or `max-height`, `none` as `auto`.
  pub(crate) fn from_max_size(size: MaxSize, sizing: &SizingContext) -> Self {
    match size {
      MaxSize::None => Self::Auto,
      MaxSize::Length(length) => length.to_layout_length(sizing),
    }
  }

  /// As taffy takes it, a `calc()` registered in `sizing`'s arena.
  pub(crate) fn to_compact_length(self, sizing: &SizingContext) -> CompactLength {
    match self {
      Self::Auto => CompactLength::auto(),
      Self::Fixed(px) => CompactLength::length(px),
      Self::Percent(percent) => CompactLength::percent(percent),
      Self::Calc { px, percent } => CompactLength::calc(
        sizing
          .calc_arena
          .register_linear(CalcLinear { px, percent }),
      ),
      Self::MinContent => CompactLength::min_content(),
      Self::MaxContent => CompactLength::max_content(),
      Self::FitContent => CompactLength::fit_content_keyword(),
      Self::Stretch => CompactLength::stretch(),
      Self::MinIntrinsic => CompactLength::min_content(),
    }
  }

  pub(crate) fn is_auto(self) -> bool {
    self == Self::Auto
  }

  /// Blink's `HasPercent`.
  pub(crate) fn has_percent(self) -> bool {
    match self {
      Self::Percent(_) => true,
      Self::Calc { percent, .. } => percent != 0.0,
      _ => false,
    }
  }

  /// Blink's `HasPercentOrStretch`.
  pub(crate) fn has_percent_or_stretch(self) -> bool {
    self.has_percent() || self == Self::Stretch
  }

  /// Blink's `HasContentOrIntrinsic`.
  pub(crate) fn is_content_or_intrinsic(self) -> bool {
    matches!(self, Self::MinContent | Self::MaxContent | Self::FitContent)
  }

  /// Blink's `ValueForLength`: a length or percentage against `basis`, `None` for a keyword or a
  /// percentage of an indefinite basis.
  pub(crate) fn resolve(self, basis: Option<LayoutUnit>) -> Option<LayoutUnit> {
    match self {
      Self::Fixed(px) => Some(LayoutUnit::from_float(px)),
      Self::Percent(percent) => {
        basis.map(|basis| LayoutUnit::from_float(basis.to_float() * percent))
      }
      Self::Calc { px, percent } => {
        basis.map(|basis| LayoutUnit::from_float(px + basis.to_float() * percent))
      }
      _ => None,
    }
  }

  /// Blink's `MinimumValueForLength`: as [`Self::resolve`], a keyword as zero and an indefinite
  /// basis as zero.
  pub(crate) fn resolve_or_zero(self, basis: Option<LayoutUnit>) -> LayoutUnit {
    self
      .resolve(Some(basis.unwrap_or(LayoutUnit::ZERO)))
      .unwrap_or(LayoutUnit::ZERO)
  }
}

/// What a box lays out with: Blink's `ComputedStyle` as layout reads it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutStyle {
  pub(crate) display: Display,
  pub(crate) position: Position,
  pub(crate) inset: Rect<LayoutLength>,
  pub(crate) size: Size<LayoutLength>,
  pub(crate) min_size: Size<LayoutLength>,
  /// `auto` stands for `none`.
  pub(crate) max_size: Size<LayoutLength>,
  pub(crate) aspect_ratio: Option<f32>,
  pub(crate) box_sizing: BoxSizing,
  pub(crate) margin: Rect<LayoutLength>,
  pub(crate) padding: Rect<LayoutLength>,
  /// The used border widths.
  pub(crate) border: Rect<f32>,
  pub(crate) overflow: Point<Overflow>,
  /// Resolved against `direction`.
  pub(crate) float: Float,
  /// Resolved against `direction`.
  pub(crate) clear: Clear,
  pub(crate) direction: Direction,
  pub(crate) contain: Contain,
  /// `Normal` stands for `auto`.
  pub(crate) justify_self: AlignItems,
  /// `Normal` stands for `auto`.
  pub(crate) align_self: AlignItems,
  pub(crate) is_replaced: bool,
}

impl Default for LayoutStyle {
  fn default() -> Self {
    Self {
      display: Display::Block,
      position: Position::Static,
      inset: Rect::all(LayoutLength::Auto),
      size: Size::all(LayoutLength::Auto),
      min_size: Size::all(LayoutLength::Auto),
      max_size: Size::all(LayoutLength::Auto),
      aspect_ratio: None,
      box_sizing: BoxSizing::ContentBox,
      margin: Rect::all(LayoutLength::Fixed(0.0)),
      padding: Rect::all(LayoutLength::Fixed(0.0)),
      border: Rect::all(0.0),
      overflow: Point::all(Overflow::Visible),
      float: Float::None,
      clear: Clear::None,
      direction: Direction::Ltr,
      contain: Contain::NONE,
      justify_self: AlignItems::Normal,
      align_self: AlignItems::Normal,
      is_replaced: false,
    }
  }
}

impl LayoutStyle {
  /// `style`'s box model, resolved against `sizing`.
  pub(crate) fn from_computed(
    style: &ComputedStyle,
    sizing: &SizingContext,
    is_replaced: bool,
  ) -> Self {
    let length = |length: Length| length.to_layout_length(sizing);
    let box_data = &style.box_data;
    let direction = style.inherited_data.direction;
    let overflow = style.resolve_overflows();

    Self {
      display: style.base_data.display,
      position: style.base_data.position,
      inset: Rect {
        top: length(style.surround_data.top),
        right: length(style.surround_data.right),
        bottom: length(style.surround_data.bottom),
        left: length(style.surround_data.left),
      },
      size: Size {
        width: LayoutLength::from_size(box_data.width, sizing),
        height: LayoutLength::from_size(box_data.height, sizing),
      },
      min_size: Size {
        width: length(box_data.min_width),
        height: length(box_data.min_height),
      },
      max_size: Size {
        width: LayoutLength::from_max_size(box_data.max_width, sizing),
        height: LayoutLength::from_max_size(box_data.max_height, sizing),
      },
      aspect_ratio: style.surround_data.aspect_ratio.into(),
      box_sizing: box_data.box_sizing,
      margin: Rect {
        top: length(box_data.margin_top),
        right: length(box_data.margin_right),
        bottom: length(box_data.margin_bottom),
        left: length(box_data.margin_left),
      },
      padding: Rect {
        top: length(box_data.padding_top),
        right: length(box_data.padding_right),
        bottom: length(box_data.padding_bottom),
        left: length(box_data.padding_left),
      },
      // Used widths are already zeroed for non-rendered styles in `make_computed`.
      border: Rect {
        top: box_data.border_top_width.to_used_px(sizing),
        right: box_data.border_right_width.to_used_px(sizing),
        bottom: box_data.border_bottom_width.to_used_px(sizing),
        left: box_data.border_left_width.to_used_px(sizing),
      },
      overflow: Point {
        x: overflow.x,
        y: overflow.y,
      },
      float: style.base_data.float.resolve(direction),
      clear: style.base_data.clear.resolve(direction),
      direction,
      contain: style.misc_data.contain,
      justify_self: style.misc_data.justify_self,
      align_self: style.misc_data.align_self,
      is_replaced,
    }
  }

  /// The taffy style of an anonymous box laid out with this style, for taffy's flexbox and grid
  /// algorithms, its other properties taffy's initial values.
  pub(crate) fn to_taffy_style(&self, sizing: &SizingContext) -> taffy::Style {
    // SAFETY: `to_compact_length` gives `auto`, a length, a percentage, or a calc registered in
    // `sizing`, which every one of these taffy types holds.
    let length_percentage_auto = |length: LayoutLength| unsafe {
      LengthPercentageAuto::from_raw(length.to_compact_length(sizing))
    };
    let dimension =
      |length: LayoutLength| unsafe { Dimension::from_raw(length.to_compact_length(sizing)) };
    let length_percentage = |length: LayoutLength| match length {
      LayoutLength::Auto => LengthPercentage::length(0.0),
      length => unsafe { LengthPercentage::from_raw(length.to_compact_length(sizing)) },
    };

    taffy::Style {
      display: self.display.into_taffy(),
      position: self.position.into_taffy(),
      direction: self.direction.into_taffy(),
      box_sizing: self.box_sizing.into_taffy(),
      inset: self.inset.map(length_percentage_auto).into_taffy(),
      size: self.size.map(dimension).into_taffy(),
      min_size: self.min_size.map(length_percentage_auto).into_taffy(),
      max_size: self.max_size.map(length_percentage_auto).into_taffy(),
      aspect_ratio: self.aspect_ratio,
      margin: self.margin.map(length_percentage_auto).into_taffy(),
      padding: self.padding.map(length_percentage).into_taffy(),
      border: self.border.map(LengthPercentage::length).into_taffy(),
      overflow: taffy::Point {
        x: self.overflow.x.into_taffy(),
        y: self.overflow.y.into_taffy(),
      },
      float: self.float,
      clear: self.clear,
      ..taffy::Style::default()
    }
  }

  /// Blink's `BoxSizingForAspectRatio`: a natural aspect ratio applies to the content box.
  pub(crate) fn box_sizing_for_aspect_ratio(&self) -> BoxSizing {
    if self.aspect_ratio.is_some() {
      return self.box_sizing;
    }

    BoxSizing::ContentBox
  }

  pub(crate) fn is_out_of_flow(&self) -> bool {
    matches!(self.position, Position::Absolute | Position::Fixed)
  }

  pub(crate) fn is_floating(&self) -> bool {
    self.float != Float::None && !self.is_out_of_flow()
  }

  /// Blink's `IsScrollContainer`: an overflow that is neither visible nor clip.
  pub(crate) fn is_scroll_container(&self) -> bool {
    [self.overflow.x, self.overflow.y]
      .iter()
      .any(|overflow| !overflow.is_clip_or_visible())
  }

  /// Blink's `ShouldApplyLayoutContainment`.
  pub(crate) fn applies_layout_containment(&self) -> bool {
    self.contain.contains(Contain::LAYOUT) || self.contain.contains(Contain::PAINT)
  }
}
