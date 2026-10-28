//! The style a box lays out with: its computed box model, with lengths in device px.

use std::rc::Rc;

#[cfg(feature = "fragmentation")]
use crate::style::{BoxDecorationBreak, BreakBetween, BreakInside};

use crate::{
  geometry::{Point, Rect, Size},
  layout::{
    floats_utils::{Clear, Float},
    grid::grid_style::{GridPosition, GridStyle},
  },
  layout_unit::LayoutUnit,
  style::{
    AlignItems, BaselineSource, BoxSizing, ComputedStyle, Contain, Direction, Display, FlexBasis,
    FlexDirection, FlexWrap, Gap, JustifyContent, Length, MaxSize, Overflow, Position,
    Size as SizeValue, SizingContext,
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

  /// Blink's `MayHavePercentDependence`: a percentage, or a `calc()` that may hold one.
  pub(crate) fn may_have_percent_dependence(self) -> bool {
    matches!(self, Self::Percent(_) | Self::Calc { .. })
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
  pub(crate) baseline_source: BaselineSource,
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
  /// Blink's `ShouldIgnoreOverflowPropertyForInlineBlockBaseline`.
  pub(crate) ignores_overflow_for_inline_block_baseline: bool,
  /// The direction `-webkit-box-orient` resolves to for a `-webkit-box`.
  pub(crate) flex_direction: FlexDirection,
  pub(crate) flex_wrap: FlexWrap,
  /// The fewest lines `flex-wrap: balance` lays out into.
  pub(crate) flex_line_count: usize,
  pub(crate) justify_content: JustifyContent,
  pub(crate) align_content: JustifyContent,
  pub(crate) align_items: AlignItems,
  pub(crate) flex_grow: f32,
  pub(crate) flex_shrink: f32,
  /// `content` stands as `max-content`.
  pub(crate) flex_basis: LayoutLength,
  /// `normal` stands as zero.
  pub(crate) row_gap: LayoutLength,
  pub(crate) column_gap: LayoutLength,
  /// `Normal` stands for `legacy`.
  pub(crate) justify_items: AlignItems,
  pub(crate) grid_row_start: GridPosition,
  pub(crate) grid_row_end: GridPosition,
  pub(crate) grid_column_start: GridPosition,
  pub(crate) grid_column_end: GridPosition,
  /// A grid container's grid.
  pub(crate) grid: Option<Rc<GridStyle>>,
  #[cfg(feature = "fragmentation")]
  pub(crate) breaks: BreakStyle,
}

/// The properties Blink's block fragmentation reads off a box.
#[cfg(feature = "fragmentation")]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct BreakStyle {
  pub(crate) break_before: BreakBetween,
  pub(crate) break_after: BreakBetween,
  pub(crate) break_inside: BreakInside,
  pub(crate) box_decoration_break: BoxDecorationBreak,
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
      baseline_source: BaselineSource::Auto,
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
      ignores_overflow_for_inline_block_baseline: false,
      flex_direction: FlexDirection::Row,
      flex_wrap: FlexWrap::NoWrap,
      flex_line_count: 1,
      justify_content: JustifyContent::Normal,
      align_content: JustifyContent::Normal,
      align_items: AlignItems::Normal,
      flex_grow: 0.0,
      flex_shrink: 1.0,
      flex_basis: LayoutLength::Auto,
      row_gap: LayoutLength::Fixed(0.0),
      column_gap: LayoutLength::Fixed(0.0),
      justify_items: AlignItems::Normal,
      grid_row_start: GridPosition::Auto,
      grid_row_end: GridPosition::Auto,
      grid_column_start: GridPosition::Auto,
      grid_column_end: GridPosition::Auto,
      grid: None,
      #[cfg(feature = "fragmentation")]
      breaks: BreakStyle::default(),
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
    let gap = |gap: Gap| match gap {
      Gap::Normal => LayoutLength::Fixed(0.0),
      Gap::Length(gap) => length(gap),
    };
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
      baseline_source: box_data.baseline_source,
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
      ignores_overflow_for_inline_block_baseline: false,
      flex_direction: style.resolved_flex_direction(),
      flex_wrap: style.misc_data.flex_wrap,
      flex_line_count: usize::from(style.misc2_data.flex_line_count.get()),
      justify_content: style.box_data.justify_content,
      align_content: style.misc_data.align_content,
      align_items: style.box_data.align_items,
      flex_grow: style.misc1_data.flex_grow.map_or(0.0, |grow| grow.0),
      flex_shrink: style.misc2_data.flex_shrink.map_or(1.0, |shrink| shrink.0),
      flex_basis: match style.misc1_data.flex_basis.unwrap_or_default() {
        FlexBasis::Content => LayoutLength::MaxContent,
        FlexBasis::Size(size) => LayoutLength::from_size(size, sizing),
      },
      row_gap: gap(style.misc3_data.row_gap),
      column_gap: gap(style.misc1_data.column_gap),
      justify_items: style.misc_data.justify_items,
      grid_row_start: GridPosition::of(&style.misc2_data.grid_row_start),
      grid_row_end: GridPosition::of(&style.misc2_data.grid_row_end),
      grid_column_start: GridPosition::of(&style.misc2_data.grid_column_start),
      grid_column_end: GridPosition::of(&style.misc2_data.grid_column_end),
      grid: style
        .base_data
        .display
        .is_grid()
        .then(|| Rc::new(GridStyle::of(style, sizing))),
      #[cfg(feature = "fragmentation")]
      breaks: BreakStyle {
        break_before: style.misc_data.break_before,
        break_after: style.misc_data.break_after,
        break_inside: style.misc_data.break_inside,
        box_decoration_break: box_data.box_decoration_break,
      },
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
