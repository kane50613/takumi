use std::borrow::Cow;
use std::marker::PhantomData;

use taffy::{Line, Point, Rect, Size};

use super::ComputedStyle;
use crate::style::SizingContext;
use crate::style::properties::*;

impl ComputedStyle {
  /// Normalize inheritable text-related values to computed values for this node.
  pub fn make_computed(&mut self, sizing: &SizingContext) {
    // `font-size` computed value is already resolved in `sizing.font_size`.
    // Keep it as css-px in style to avoid re-resolving descendant inheritance.
    let dpr = sizing.viewport.device_pixel_ratio;
    self.font_size = if dpr > 0.0 {
      FontSize::Length(Length::Px(sizing.font_size / dpr))
    } else {
      FontSize::Length(Length::Px(sizing.font_size))
    };

    self.make_computed_values(sizing);

    // https://www.w3.org/TR/css-display-3/#transformations
    // Elements with position: absolute or fixed are blockified
    if self.position.is_out_of_flow() || self.float != Float::None {
      self.display.blockify();
    }
  }

  pub fn is_invisible(&self) -> bool {
    self.opacity.0 == 0.0 || self.display == Display::None || self.visibility == Visibility::Hidden
  }

  pub fn is_z_index_applicable(&self, is_flex_or_grid_item: bool) -> bool {
    !matches!(self.z_index, ZIndex::Auto) && (self.position.is_positioned() || is_flex_or_grid_item)
  }

  pub fn participates_in_positioned_paint_bucket(&self, is_flex_or_grid_item: bool) -> bool {
    self.position.is_positioned() || self.is_z_index_applicable(is_flex_or_grid_item)
  }

  pub fn creates_stacking_context(
    &self,
    border_box: Size<f32>,
    sizing: &SizingContext,
    is_flex_or_grid_item: bool,
  ) -> bool {
    self.isolation == Isolation::Isolate
      || self.is_z_index_applicable(is_flex_or_grid_item)
      || self.has_non_identity_transform(border_box, sizing)
      || self.needs_offscreen_compositing()
  }

  pub fn needs_offscreen_compositing(&self) -> bool {
    self.isolation == Isolation::Isolate
      || *self.opacity < 1.0
      || !self.filter.is_empty()
      || !self.backdrop_filter.is_empty()
      || self.mix_blend_mode != BlendMode::Normal
      || self.clip_path.is_some()
      || self.mask_image.as_ref().is_some_and(|images| {
        images
          .iter()
          .any(|image| !matches!(image, BackgroundImage::None))
      })
  }

  /// Builds the element's local affine transform around its transform-origin
  /// (CSS Transforms Level 2 order: `T(origin) * translate * rotate * scale *
  /// transform * T(-origin)`).
  pub fn local_transform(&self, border_box: Size<f32>, sizing: &SizingContext) -> Affine {
    let origin = self.transform_origin.to_point(sizing, border_box);
    let mut local = Affine::translation(origin.x, origin.y);

    if self.translate != SpacePair::default() {
      local *= Affine::translation(
        self.translate.x.to_px(sizing, border_box.width),
        self.translate.y.to_px(sizing, border_box.height),
      );
    }
    if let Some(rotate) = self.rotate {
      local *= Affine::rotation(rotate);
    }
    if self.scale != SpacePair::default() {
      local *= Affine::scale(self.scale.x.0, self.scale.y.0);
    }
    if let Some(node_transform) = &self.transform {
      local *= Affine::from_transforms(node_transform.iter(), sizing, border_box);
    }
    local *= Affine::translation(-origin.x, -origin.y);
    local
  }

  pub fn has_non_identity_transform(&self, border_box: Size<f32>, sizing: &SizingContext) -> bool {
    !self.local_transform(border_box, sizing).is_identity()
  }

  pub fn resolve_overflows(&self) -> SpacePair<Overflow> {
    SpacePair::from_pair(self.overflow_x, self.overflow_y)
  }

  pub fn clips_overflow(&self) -> bool {
    self.resolve_overflows().should_clip_content()
  }

  pub fn ellipsis_char(&self) -> &str {
    const ELLIPSIS_CHAR: &str = "…";

    match &self.text_overflow {
      TextOverflow::Ellipsis => return ELLIPSIS_CHAR,
      TextOverflow::Custom(custom) => return custom.as_str(),
      _ => {}
    }

    if let Some(clamp) = &self
      .line_clamp
      .as_ref()
      .and_then(|clamp| clamp.ellipsis.as_deref())
    {
      return clamp;
    }

    ELLIPSIS_CHAR
  }

  pub fn text_wrap_mode_and_line_clamp(&self) -> (TextWrapMode, Option<Cow<'_, LineClamp>>) {
    let mut text_wrap_mode = self.text_wrap_mode;
    let mut line_clamp = self.line_clamp.as_ref().map(Cow::Borrowed);

    // Special case: when nowrap + ellipsis, parley will layout all the text even when it overflows.
    // So we need to use a fixed line clamp of 1 instead.
    if text_wrap_mode == TextWrapMode::NoWrap && self.text_overflow == TextOverflow::Ellipsis {
      line_clamp = Some(Cow::Owned(self.single_line_ellipsis_clamp()));

      text_wrap_mode = TextWrapMode::Wrap;
    }

    (text_wrap_mode, line_clamp)
  }

  #[inline]
  fn single_line_ellipsis_clamp(&self) -> LineClamp {
    LineClamp {
      count: 1,
      ellipsis: Some(self.ellipsis_char().to_string()),
    }
  }

  #[inline]
  fn grid_template(
    components: &Option<GridTemplateComponents>,
    sizing: &SizingContext,
  ) -> (Vec<taffy::GridTemplateComponent<String>>, Vec<Vec<String>>) {
    components.as_deref().map_or_else(
      || (Vec::new(), vec![Vec::new()]),
      |components| collect_components_and_names(components, sizing),
    )
  }

  #[inline]
  pub fn resolved_text_decoration_thickness(
    &self,
    sizing: &SizingContext,
  ) -> SizedTextDecorationThickness {
    match self.text_decoration_thickness {
      TextDecorationThickness::Length(Length::Auto) | TextDecorationThickness::FromFont => {
        SizedTextDecorationThickness::FromFont
      }
      TextDecorationThickness::Length(thickness) => {
        SizedTextDecorationThickness::Value(thickness.to_px(sizing, sizing.font_size))
      }
    }
  }

  pub fn to_taffy_style(&self, sizing: &SizingContext) -> taffy::Style {
    // Convert grid templates and associated line names
    let (grid_template_columns, grid_template_column_names) =
      Self::grid_template(&self.grid_template_columns, sizing);
    let (grid_template_rows, grid_template_row_names) =
      Self::grid_template(&self.grid_template_rows, sizing);

    taffy::Style {
      float: self.float.resolve(self.direction),
      clear: self.clear.resolve(self.direction),
      direction: self.direction.into(),
      box_sizing: self.box_sizing.into(),
      size: Size {
        width: self.width,
        height: self.height,
      }
      .map(|length| length.resolve_to_dimension(sizing)),
      border: Rect {
        top: if !self.border_top_style.is_rendered() {
          Length::default()
        } else {
          self.border_top_width
        },
        right: if !self.border_right_style.is_rendered() {
          Length::default()
        } else {
          self.border_right_width
        },
        bottom: if !self.border_bottom_style.is_rendered() {
          Length::default()
        } else {
          self.border_bottom_width
        },
        left: if !self.border_left_style.is_rendered() {
          Length::default()
        } else {
          self.border_left_width
        },
      }
      .map(|border| border.resolve_to_length_percentage(sizing)),
      padding: Rect {
        top: self.padding_top,
        right: self.padding_right,
        bottom: self.padding_bottom,
        left: self.padding_left,
      }
      .map(|padding| padding.resolve_to_length_percentage(sizing)),
      inset: if self.position == Position::Static {
        Rect::auto()
      } else {
        Rect {
          top: self.top,
          right: self.right,
          bottom: self.bottom,
          left: self.left,
        }
        .map(|inset| inset.resolve_to_length_percentage_auto(sizing))
      },
      margin: Rect {
        top: self.margin_top,
        right: self.margin_right,
        bottom: self.margin_bottom,
        left: self.margin_left,
      }
      .map(|margin| margin.resolve_to_length_percentage_auto(sizing)),
      display: self.display.into(),
      flex_direction: self.flex_direction.into(),
      position: self.position.into(),
      justify_content: self.justify_content.into(),
      align_content: self.align_content.into(),
      justify_items: self.justify_items.into(),
      flex_grow: self.flex_grow.map(|grow| grow.0).unwrap_or(0.0),
      align_items: self.align_items.into(),
      gap: Size {
        width: self.column_gap.resolve_to_length_percentage(sizing),
        height: self.row_gap.resolve_to_length_percentage(sizing),
      },
      flex_basis: self
        .flex_basis
        .unwrap_or(Length::Auto)
        .resolve_to_dimension(sizing),
      flex_shrink: self.flex_shrink.map(|shrink| shrink.0).unwrap_or(1.0),
      flex_wrap: self.flex_wrap.into(),
      min_size: Size {
        width: self.min_width,
        height: self.min_height,
      }
      .map(|length| length.resolve_to_dimension(sizing)),
      max_size: Size {
        width: self.max_width,
        height: self.max_height,
      }
      .map(|length| length.resolve_to_dimension(sizing)),
      grid_auto_columns: self
        .grid_auto_columns
        .as_ref()
        .map_or_else(Vec::new, |tracks| {
          tracks
            .iter()
            .map(|track| track.to_min_max(sizing))
            .collect()
        }),
      grid_auto_rows: self
        .grid_auto_rows
        .as_ref()
        .map_or_else(Vec::new, |tracks| {
          tracks
            .iter()
            .map(|track| track.to_min_max(sizing))
            .collect()
        }),
      grid_auto_flow: self.grid_auto_flow.into(),
      grid_column: Line {
        start: self.grid_column_start.clone().into(),
        end: self.grid_column_end.clone().into(),
      },
      grid_row: Line {
        start: self.grid_row_start.clone().into(),
        end: self.grid_row_end.clone().into(),
      },
      grid_template_columns,
      grid_template_rows,
      grid_template_column_names,
      grid_template_row_names,
      grid_template_areas: self
        .grid_template_areas
        .as_ref()
        .cloned()
        .unwrap_or_default()
        .into(),
      aspect_ratio: self.aspect_ratio.into(),
      align_self: self.align_self.into(),
      justify_self: self.justify_self.into(),
      overflow: Point::from(self.resolve_overflows()).map(Into::into),
      dummy: PhantomData,
      item_is_table: false,
      item_is_replaced: false,
      scrollbar_width: 0.0,
      text_align: taffy::TextAlign::Auto,
    }
  }
}
