//! `use` instances, `symbol` viewports, and nested `svg` elements, after Blink's
//! `SVGUseElement::BuildShadowTree` and `LayoutSVGViewportContainer`.

use std::rc::Rc;

use super::{
  Scope, Styled, SvgLayout,
  object::{Container, ContextSource, LayoutClip, LayoutObject},
  view_box::PreserveAspectRatio,
};
use crate::{
  context::RenderContext,
  geometry::{Rect, Size},
  resources::svg_size::ViewBox,
  style::{Affine, Length, Overflow, Size as SizeValue},
  svg::{element::SvgElementKind, length::LengthAxis},
};

impl SvgLayout<'_> {
  /// The element a `use` instantiates, none when it names nothing drawable or would contain
  /// itself.
  fn use_target(&self, use_index: usize) -> Option<usize> {
    let target = self.first_id_target(use_index)?;

    if target == use_index
      || matches!(
        self.document.elements[target].kind,
        SvgElementKind::Unknown
          | SvgElementKind::Style
          | SvgElementKind::ForeignObject
          | SvgElementKind::CharacterData
      )
      || self.active.contains(&use_index)
    {
      return None;
    }

    // None of the target's descendants may be a `use` of this one or of the target.
    let mut stack = self.document.element_children(target);

    while let Some(descendant) = stack.pop() {
      if self.document.elements[descendant].kind == SvgElementKind::Use
        && let Some(inner) = self.first_id_target(descendant)
        && (inner == use_index || inner == target)
      {
        return None;
      }

      stack.extend(self.document.element_children(descendant));
    }

    Some(target)
  }

  /// The element an `href` names, where several elements share an `id` the first one counts.
  fn first_id_target(&self, index: usize) -> Option<usize> {
    let fragment = self.attribute(index, "href")?.trim().strip_prefix('#')?;

    self.document.first_ids.get(fragment).copied()
  }

  pub(super) fn convert_use(&mut self, styled: &Styled, scope: &Scope, parent: &mut Container) {
    let Some(target) = self.use_target(styled.index) else {
      return;
    };

    // A `symbol` is not drawn in a clip path, which saves making a clip path for its viewport.
    if scope.parent_clip_path.is_some()
      && self.document.elements[target].kind == SvgElementKind::Symbol
    {
      return;
    }

    self.active.push(styled.index);
    self.deeper(|layout| layout.expand_use(styled, target, scope, parent));
    self.active.pop();
  }

  fn expand_use(&mut self, styled: &Styled, target: usize, scope: &Scope, parent: &mut Container) {
    let mut use_scope = scope.clone();
    let fill = self.resolve_fill(styled, true, scope).map(|mut fill| {
      fill.context = Some(ContextSource::UseNode);
      fill
    });
    let stroke = self.resolve_stroke(styled, true, scope).map(|mut stroke| {
      stroke.context = Some(ContextSource::UseNode);
      stroke
    });

    use_scope.context_paints = Some((fill, stroke));

    // The original transform is needed to set up the clip path.
    let mut original_transform = self.resolve_transform(styled, scope);
    let geometry = &styled.style().svg_geometry_data;
    let x = self.px(styled, &use_scope, geometry.x, LengthAxis::X);
    let y = self.px(styled, &use_scope, geometry.y, LengthAxis::Y);
    let mut new_transform = Affine::translation(x, y);
    let linked_to_symbol = self.document.elements[target].kind == SvgElementKind::Symbol;

    if linked_to_symbol {
      let symbol = self.styled(target, &styled.context);

      // Relative units resolve against the size of the `use`, not of the viewport around it.
      let width = if self.has_attribute(styled.index, "width") {
        self.attribute_px(
          styled,
          &use_scope,
          "width",
          LengthAxis::X,
          Length::Percentage(100.0),
        )
      } else {
        use_scope.view_box.width()
      };
      let height = if self.has_attribute(styled.index, "height") {
        self.attribute_px(
          styled,
          &use_scope,
          "height",
          LengthAxis::Y,
          Length::Percentage(100.0),
        )
      } else {
        use_scope.view_box.height()
      };

      use_scope.view_box = Rect::positive_from_xywh(
        use_scope.view_box.x(),
        use_scope.view_box.y(),
        width,
        height,
      )
      .unwrap_or(use_scope.view_box);

      if let Some(transform) = self.view_box_transform(styled, &symbol, scope) {
        new_transform *= transform;
      }

      if let Some(clip_rect) = self.clip_rect(styled, &symbol, scope) {
        let Some(mut clipped) = self.clip_container(original_transform, clip_rect, parent) else {
          return;
        };

        clipped.abs_transform = parent.abs_transform;

        let group = self.convert_group(styled, &use_scope, true, &mut clipped, |layout, group| {
          layout.convert_use_content(
            &symbol,
            &symbol.context,
            new_transform,
            &use_scope,
            false,
            group,
          );
        });

        if let Some(mut group) = group {
          // The clip container carries the transform already.
          clipped.context_element = true;
          group.transform = Affine::IDENTITY;
          clipped
            .children
            .push(LayoutObject::Container(Box::new(group)));
        }

        if clipped.children.is_empty() {
          return;
        }

        clipped.finish();
        clipped.settle_use_paints();
        parent
          .children
          .push(LayoutObject::Container(Box::new(clipped)));
        return;
      }
    }

    original_transform *= new_transform;

    if linked_to_symbol {
      let symbol = self.styled(target, &styled.context);
      let group = self.convert_group(styled, &use_scope, false, parent, |layout, group| {
        layout.convert_use_content(
          &symbol,
          &symbol.context,
          original_transform,
          &use_scope,
          false,
          group,
        );
      });

      if let Some(mut group) = group {
        group.context_element = true;
        group.transform = Affine::IDENTITY;
        group.settle_use_paints();
        parent
          .children
          .push(LayoutObject::Container(Box::new(group)));
      }

      return;
    }

    if self.document.elements[target].kind == SvgElementKind::Svg {
      // The size of the `use` stands in for the size of the `svg`, and each `use` starts afresh.
      use_scope.use_size = (None, None);

      if self.has_attribute(styled.index, "width") {
        use_scope.use_size.0 = Some(self.attribute_px(
          styled,
          &use_scope,
          "width",
          LengthAxis::X,
          Length::Percentage(100.0),
        ));
      }

      if self.has_attribute(styled.index, "height") {
        use_scope.use_size.1 = Some(self.attribute_px(
          styled,
          &use_scope,
          "height",
          LengthAxis::Y,
          Length::Percentage(100.0),
        ));
      }
    }

    let group = self.convert_use_group(
      styled,
      original_transform,
      &use_scope,
      true,
      parent,
      |layout, group| {
        if use_scope.parent_clip_path.is_some() {
          layout.convert_clip_path_child(target, &styled.context, &use_scope, group);
        } else {
          layout.convert_element(target, &styled.context, &use_scope, group);
        }
      },
    );

    if let Some(mut group) = group {
      group.settle_use_paints();
      parent
        .children
        .push(LayoutObject::Container(Box::new(group)));
    }
  }

  /// The group a `use` makes with `transform`, filled by `fill`. Its transform replaces the one
  /// the element would have had, the way an instance sits under the `use` element's own.
  fn convert_use_group(
    &mut self,
    styled: &Styled,
    transform: Affine,
    scope: &Scope,
    is_context_element: bool,
    parent: &mut Container,
    fill: impl FnOnce(&mut Self, &mut Container),
  ) -> Option<Container> {
    let parent_abs_transform = parent.abs_transform;
    let required = transform.to_cols_array() != Affine::IDENTITY.to_cols_array();

    self.convert_group(styled, scope, required, parent, |layout, group| {
      group.transform = transform;
      group.abs_transform = parent_abs_transform * transform;
      group.context_element = is_context_element;
      fill(layout, group);
    })
  }

  /// Lays out the children of `source`, in a group made from `source` that carries `transform`,
  /// into `parent`.
  fn convert_use_content(
    &mut self,
    source: &Styled,
    children_context: &RenderContext,
    transform: Affine,
    scope: &Scope,
    is_context_element: bool,
    parent: &mut Container,
  ) {
    let group = self.convert_use_group(
      source,
      transform,
      scope,
      is_context_element,
      parent,
      |layout, group| {
        if scope.parent_clip_path.is_some() {
          layout.convert_clip_path_children(source.index, children_context, scope, group);
        } else {
          layout.convert_children(source.index, children_context, scope, group);
        }
      },
    );

    if let Some(group) = group {
      parent
        .children
        .push(LayoutObject::Container(Box::new(group)));
    }
  }

  /// A nested `svg`: a viewport with its own coordinate system, clipped to its bounds.
  pub(super) fn convert_nested_svg(
    &mut self,
    styled: &Styled,
    scope: &Scope,
    parent: &mut Container,
  ) {
    // The original transform is needed to set up the clip path.
    let mut original_transform = self.resolve_transform(styled, scope);
    let geometry = &styled.style().svg_geometry_data;
    let x = self.px(styled, scope, geometry.x, LengthAxis::X);
    let y = self.px(styled, scope, geometry.y, LengthAxis::Y);
    let mut new_transform = Affine::translation(x, y);

    if let Some(transform) = self.view_box_transform(styled, styled, scope) {
      new_transform *= transform;
    }

    // The nested viewport is the box the `viewBox` names, or else the `svg`'s own rectangle.
    let mut inner_scope = scope.clone();

    inner_scope.view_box = match self
      .attribute(styled.index, "viewBox")
      .and_then(ViewBox::parse)
    {
      Some(view_box) => {
        Rect::positive_from_xywh(view_box.x, view_box.y, view_box.width, view_box.height)
          .unwrap_or(scope.view_box)
      }
      None => {
        let (mut width, mut height) = self.viewport_size(styled, scope);

        // A `use` that sizes the `svg` overrides the size the `svg` has.
        width = scope.use_size.0.unwrap_or(width);
        height = scope.use_size.1.unwrap_or(height);

        Rect::positive_from_xywh(x, y, width, height).unwrap_or(scope.view_box)
      }
    };

    if let Some(clip_rect) = self.clip_rect(styled, styled, scope) {
      let Some(mut clipped) = self.clip_container(original_transform, clip_rect, parent) else {
        return;
      };

      clipped.abs_transform = parent.abs_transform * original_transform;
      self.convert_use_content(
        styled,
        &styled.context,
        new_transform,
        &inner_scope,
        false,
        &mut clipped,
      );
      clipped.finish();
      parent
        .children
        .push(LayoutObject::Container(Box::new(clipped)));
    } else {
      original_transform *= new_transform;
      self.convert_use_content(
        styled,
        &styled.context,
        original_transform,
        &inner_scope,
        false,
        parent,
      );
    }
  }

  /// A container that clips its content to `rect`, which a viewport needs, as a clip path on the
  /// element itself would be moved by the element's transform.
  fn clip_container(
    &self,
    transform: Affine,
    rect: Rect<f32>,
    parent: &Container,
  ) -> Option<Container> {
    let mut container = Container::new(parent.abs_transform);

    container.transform = transform;
    container.clip_path = Some(Rc::new(LayoutClip {
      transform: Affine::IDENTITY,
      clip_path: None,
      root: Container::filled_rectangle(rect)?,
    }));
    Some(container)
  }

  /// Where a viewport clips: its rectangle, unless its overflow is visible or it has no size.
  fn clip_rect(&self, viewport: &Styled, content: &Styled, scope: &Scope) -> Option<Rect<f32>> {
    // There is no need to clip content with visible overflow.
    if content.style().base_data.overflow_x == Overflow::Visible {
      return None;
    }

    // A nested `svg` with only a `viewBox` has no rectangle to clip to, unless a `use` sizes it.
    if viewport.kind == SvgElementKind::Svg
      && scope.use_size == (None, None)
      && !(self.has_size(viewport, LengthAxis::X) && self.has_size(viewport, LengthAxis::Y))
    {
      return None;
    }

    let geometry = &viewport.style().svg_geometry_data;
    let x = self.px(viewport, scope, geometry.x, LengthAxis::X);
    let y = self.px(viewport, scope, geometry.y, LengthAxis::Y);
    let (mut width, mut height) = self.viewport_size(viewport, scope);

    if viewport.kind == SvgElementKind::Svg {
      width = scope.use_size.0.unwrap_or(width);
      height = scope.use_size.1.unwrap_or(height);
    }

    if !(width > 0.0 && width.is_finite() && height > 0.0 && height.is_finite()) {
      return None;
    }

    Rect::positive_from_xywh(x, y, width, height)
  }

  /// Whether the element sets its width or height, as an attribute of a `use` or the property of
  /// an `svg`.
  fn has_size(&self, styled: &Styled, axis: LengthAxis) -> bool {
    if styled.kind == SvgElementKind::Svg {
      let style = &styled.style().box_data;

      return !match axis {
        LengthAxis::X => style.width,
        _ => style.height,
      }
      .is_auto();
    }

    self.has_attribute(
      styled.index,
      if axis == LengthAxis::X {
        "width"
      } else {
        "height"
      },
    )
  }

  /// The width and height of a `use` or `svg`, a hundred percent of the viewport where unset.
  fn viewport_size(&self, styled: &Styled, scope: &Scope) -> (f32, f32) {
    let hundred_percent = Length::Percentage(100.0);

    if styled.kind == SvgElementKind::Svg {
      let style = &styled.style().box_data;
      let size = |size: SizeValue, axis| match size {
        SizeValue::Length(Length::Auto)
        | SizeValue::MinContent
        | SizeValue::MaxContent
        | SizeValue::FitContent
        | SizeValue::Stretch => self.px(styled, scope, hundred_percent, axis),
        SizeValue::Length(length) => self.px(styled, scope, length, axis),
      };

      return (
        size(style.width, LengthAxis::X),
        size(style.height, LengthAxis::Y),
      );
    }

    (
      self.attribute_px(styled, scope, "width", LengthAxis::X, hundred_percent),
      self.attribute_px(styled, scope, "height", LengthAxis::Y, hundred_percent),
    )
  }

  /// The transform that fits the `viewBox` of `linked` to the size of the viewport `node` makes.
  fn view_box_transform(&self, node: &Styled, linked: &Styled, scope: &Scope) -> Option<Affine> {
    let (mut width, mut height) = self.viewport_size(node, scope);

    if node.kind == SvgElementKind::Svg {
      width = scope.use_size.0.unwrap_or(width);
      height = scope.use_size.1.unwrap_or(height);
    }

    if !(width > 0.0 && width.is_finite() && height > 0.0 && height.is_finite()) {
      return None;
    }

    let view_box = self
      .attribute(linked.index, "viewBox")
      .and_then(ViewBox::parse)?;
    let aspect = self
      .attribute(linked.index, "preserveAspectRatio")
      .and_then(PreserveAspectRatio::parse)
      .unwrap_or_default();

    Some(view_box.to_transform(aspect, Size::new(width, height)))
  }
}
