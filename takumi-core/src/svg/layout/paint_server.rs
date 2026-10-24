//! Gradients and patterns, after Blink's `LayoutSVGResourceLinearGradient`,
//! `LayoutSVGResourceRadialGradient`, and `LayoutSVGResourcePattern`.

use std::rc::Rc;

use super::{
  Scope, SvgLayout,
  bounds::ApproxEq,
  object::{Container, GradientGeometry, GradientPaint, LayoutObject, Paint, PatternPaint, Units},
  view_box::PreserveAspectRatio,
};
use crate::{
  geometry::{Point, Rect, Size},
  path_data::Scanner,
  resources::{
    image::{SvgGradientStop, SvgSpreadMethod},
    svg_size::ViewBox,
  },
  style::{Affine, Color, Length},
  svg::{
    element::SvgElementKind,
    length::{LengthAxis, SvgLengthContext},
  },
};

/// What a paint server element paints with: the server, or the one color it degenerates to.
pub(super) enum PaintServer {
  Server(Paint),
  Color { color: Color, opacity: f32 },
}

impl SvgLayout<'_> {
  /// The paint the server element at `index` makes.
  pub(super) fn paint_server(&mut self, index: usize, scope: &Scope) -> Option<PaintServer> {
    if let Some(paint) = self.paint_servers.get(&index) {
      return Some(PaintServer::Server(paint.clone()));
    }

    let server = match self.document.elements[index].kind {
      SvgElementKind::LinearGradient => self.linear_gradient(index, scope),
      SvgElementKind::RadialGradient => self.radial_gradient(index, scope),
      SvgElementKind::Pattern => self
        .guarded(index, |layout| layout.pattern(index, scope))
        .flatten(),
      _ => None,
    }?;

    if let PaintServer::Server(paint) = &server {
      self.paint_servers.insert(index, paint.clone());
    }

    Some(server)
  }

  fn linear_gradient(&mut self, index: usize, scope: &Scope) -> Option<PaintServer> {
    let stops = self.gradient_stops(self.gradient_with_stops(index)?);

    if stops.len() < 2 {
      return stops_to_color(&stops);
    }

    let units = self.units(index, "gradientUnits", Units::ObjectBoundingBox);
    let number = |layout: &Self, name: &str, default: Length| {
      layout.resolve_number(index, name, units, scope, default)
    };
    let geometry = GradientGeometry::Linear {
      start: Point::new(
        number(self, "x1", Length::zero()),
        number(self, "y1", Length::zero()),
      ),
      end: Point::new(
        number(self, "x2", Length::Percentage(100.0)),
        number(self, "y2", Length::zero()),
      ),
    };

    Some(PaintServer::Server(Paint::Gradient(Rc::new(
      GradientPaint {
        geometry,
        units,
        transform: self.own_transform(index, scope),
        spread: self.spread_method(index),
        stops,
      },
    ))))
  }

  fn radial_gradient(&mut self, index: usize, scope: &Scope) -> Option<PaintServer> {
    let stops = self.gradient_stops(self.gradient_with_stops(index)?);

    if stops.len() < 2 {
      return stops_to_color(&stops);
    }

    let units = self.units(index, "gradientUnits", Units::ObjectBoundingBox);
    let number =
      |name: &str, default: Length| self.resolve_number(index, name, units, scope, default);
    let radius = number("r", Length::Percentage(50.0));
    let focal_radius = number("fr", Length::zero());

    // A radius of zero paints the area with the color and opacity of the last stop.
    // https://www.w3.org/TR/SVG11/pservers.html#RadialGradientElementRAttribute
    if !(radius > 0.0 && radius.is_finite()) {
      let stop = stops.last()?;

      return Some(PaintServer::Color {
        color: stop.color,
        opacity: stop.opacity,
      });
    }

    let cx = number("cx", Length::Percentage(50.0));
    let cy = number("cy", Length::Percentage(50.0));
    let fx = number("fx", Length::Px(cx));
    let fy = number("fy", Length::Px(cy));
    let geometry = GradientGeometry::Radial {
      center: Point::new(cx, cy),
      radius,
      focal: Point::new(fx, fy),
      focal_radius: if focal_radius > 0.0 && focal_radius.is_finite() {
        focal_radius
      } else {
        0.0
      },
    };

    Some(PaintServer::Server(Paint::Gradient(Rc::new(
      GradientPaint {
        geometry,
        units,
        transform: self.own_transform(index, scope),
        spread: self.spread_method(index),
        stops,
      },
    ))))
  }

  fn pattern(&mut self, index: usize, scope: &Scope) -> Option<PaintServer> {
    let with_children = self.pattern_with_children(index)?;
    let view_box_source = self.resolve_attr(index, "viewBox");
    let aspect_source = self.resolve_attr(index, "preserveAspectRatio");
    let view_box = self
      .attribute(view_box_source, "viewBox")
      .and_then(ViewBox::parse)
      .map(|view_box| {
        let aspect = self
          .attribute(aspect_source, "preserveAspectRatio")
          .and_then(PreserveAspectRatio::parse)
          .unwrap_or_default();

        (view_box, aspect)
      });
    let units = self.units(index, "patternUnits", Units::ObjectBoundingBox);
    let content_units = self.units(index, "patternContentUnits", Units::UserSpaceOnUse);
    let transform = self.own_transform(index, scope);
    let number = |name: &str| self.resolve_number(index, name, units, scope, Length::zero());
    let rect =
      Rect::positive_from_xywh(number("x"), number("y"), number("width"), number("height"))?;
    let context = self.styles.in_place(with_children);
    let mut root = Container::new(Affine::IDENTITY);

    // The viewBox applies now only when no bounding box is needed to place the content.
    if let Some((view_box, aspect)) = view_box
      && units == Units::UserSpaceOnUse
      && content_units == Units::UserSpaceOnUse
    {
      let transform = view_box.to_transform(aspect, Size::new(rect.width(), rect.height()));
      let mut group = Container::new(transform);

      group.transform = transform;
      self.convert_children(with_children, &context, scope, &mut group);

      if !group.has_children() {
        return None;
      }

      group.finish();
      root.children.push(LayoutObject::Container(Box::new(group)));
    } else {
      self.convert_children(with_children, &context, scope, &mut root);

      if !root.has_children() {
        return None;
      }
    }

    root.finish();

    Some(PaintServer::Server(Paint::Pattern(Rc::new(PatternPaint {
      units,
      content_units,
      transform,
      rect,
      view_box,
      root,
    }))))
  }

  /// The elements an `href` chain visits from the one at `index`, stopping before a cycle.
  fn href_chain(&self, index: usize) -> Vec<usize> {
    let mut chain = vec![index];
    let mut current = index;

    while let Some(link) = self.href_target(current) {
      if link == current || link == index || chain.contains(&link) {
        break;
      }

      chain.push(link);
      current = link;
    }

    chain
  }

  /// The gradient the `href` chain reaches that has stops, none when the chain meets a non-gradient.
  fn gradient_with_stops(&self, index: usize) -> Option<usize> {
    for link in self.href_chain(index) {
      if !self.document.elements[link].kind.is_gradient() {
        return None;
      }

      if self.has_child_of_kind(link, SvgElementKind::Stop) {
        return Some(link);
      }
    }

    None
  }

  fn pattern_with_children(&self, index: usize) -> Option<usize> {
    for link in self.href_chain(index) {
      if self.document.elements[link].kind != SvgElementKind::Pattern {
        return None;
      }

      if !self.document.elements[link].children.is_empty() {
        return Some(link);
      }
    }

    None
  }

  fn has_child_of_kind(&self, index: usize, kind: SvgElementKind) -> bool {
    self.document.elements[index]
      .children
      .iter()
      .any(|child| self.document.elements[*child].kind == kind)
  }

  /// The element of the `href` chain that sets the attribute, or the element itself.
  fn resolve_attr(&self, index: usize, name: &str) -> usize {
    if self.has_attribute(index, name) {
      return index;
    }

    let kind = self.document.elements[index].kind;
    let inheritable = |source: SvgElementKind| match kind {
      SvgElementKind::LinearGradient => match name {
        "x1" | "y1" | "x2" | "y2" => source == SvgElementKind::LinearGradient,
        "gradientUnits" | "spreadMethod" | "gradientTransform" => source.is_gradient(),
        _ => false,
      },
      SvgElementKind::RadialGradient => match name {
        "cx" | "cy" | "r" | "fx" | "fy" | "fr" => source == SvgElementKind::RadialGradient,
        "gradientUnits" | "spreadMethod" | "gradientTransform" => source.is_gradient(),
        _ => false,
      },
      SvgElementKind::Pattern => source == SvgElementKind::Pattern,
      SvgElementKind::Filter => source == SvgElementKind::Filter,
      _ => false,
    };

    for link in self.href_chain(index) {
      if !inheritable(self.document.elements[link].kind) {
        break;
      }

      if self.has_attribute(link, name) {
        return link;
      }
    }

    index
  }

  fn units(&self, index: usize, name: &str, default: Units) -> Units {
    match self.attribute(self.resolve_attr(index, name), name) {
      Some("userSpaceOnUse") => Units::UserSpaceOnUse,
      Some("objectBoundingBox") => Units::ObjectBoundingBox,
      _ => default,
    }
  }

  fn spread_method(&self, index: usize) -> SvgSpreadMethod {
    match self.attribute(self.resolve_attr(index, "spreadMethod"), "spreadMethod") {
      Some("reflect") => SvgSpreadMethod::Reflect,
      Some("repeat") => SvgSpreadMethod::Repeat,
      _ => SvgSpreadMethod::Pad,
    }
  }

  /// The element's own `gradientTransform` or `patternTransform`, moved by its transform origin.
  fn own_transform(&self, index: usize, scope: &Scope) -> Affine {
    self.resolve_transform(&self.styled_in_place(index), scope)
  }

  /// A length attribute of the server, read from the nearest element of the `href` chain that
  /// sets it, in user units or in fractions of the bounding box.
  pub(super) fn resolve_number(
    &self,
    index: usize,
    name: &str,
    units: Units,
    scope: &Scope,
    default: Length,
  ) -> f32 {
    let source = self.resolve_attr(index, name);
    let styled = self.styled_in_place(source);
    let length = self
      .attribute(source, name)
      .and_then(Length::from_svg_attribute)
      .unwrap_or(default);

    if units == Units::ObjectBoundingBox {
      return SvgLengthContext::resolve_in_bounding_box(length, styled.sizing());
    }

    let axis = match name {
      "x" | "x1" | "x2" | "cx" | "fx" | "width" => LengthAxis::X,
      "y" | "y1" | "y2" | "cy" | "fy" | "height" => LengthAxis::Y,
      _ => LengthAxis::Other,
    };

    self.px(&styled, scope, length, axis)
  }

  fn gradient_stops(&self, gradient: usize) -> Vec<SvgGradientStop> {
    let mut stops: Vec<SvgGradientStop> = Vec::new();
    let mut previous_offset = 0.0_f32;

    for stop in self.document.elements[gradient].children.clone() {
      if self.document.elements[stop].kind != SvgElementKind::Stop {
        continue;
      }

      let offset = self
        .attribute(stop, "offset")
        .and_then(stop_offset)
        .unwrap_or(previous_offset);

      previous_offset = offset;

      let styled = self.styled_in_place(stop);
      let color = styled
        .style()
        .svg_stop_data
        .stop_color
        .resolve(styled.context.current_color);
      let opacity =
        f32::from(color.0[3]) / 255.0 * styled.style().svg_stop_data.stop_opacity.0.clamp(0.0, 1.0);

      stops.push(SvgGradientStop {
        offset: offset.clamp(0.0, 1.0),
        color: Color([color.0[0], color.0[1], color.0[2], 255]),
        opacity,
      });
    }

    spread_equal_offsets(&mut stops);
    stops
  }

  /// Runs `f` unless the element at `index` is already being laid out, which would recurse.
  pub(super) fn guarded<R>(&mut self, index: usize, f: impl FnOnce(&mut Self) -> R) -> Option<R> {
    if self.active.contains(&index) {
      return None;
    }

    self.active.push(index);

    let result = self.deeper(f);

    self.active.pop();
    result
  }
}

/// A stop's `offset`: a number, or a percentage that is a hundredth of one. Read in `f64`, as
/// usvg does, so a percentage keeps the float it has always rounded to.
fn stop_offset(text: &str) -> Option<f32> {
  let mut scanner = Scanner::new(text);
  let number = scanner.number()?;
  let offset = if scanner.accept(b'%') {
    number / 100.0
  } else {
    number
  };

  scanner.skip_spaces();
  scanner.at_end().then_some(offset as f32)
}

fn stops_to_color(stops: &[SvgGradientStop]) -> Option<PaintServer> {
  stops.first().map(|stop| PaintServer::Color {
    color: stop.color,
    opacity: stop.opacity,
  })
}

/// Gives the stops that share an offset distinct ones, as usvg does.
fn spread_equal_offsets(stops: &mut Vec<SvgGradientStop>) {
  // Remove the stop in the middle of three at one offset.
  if stops.len() >= 3 {
    let mut index = 0;

    while index < stops.len() - 2 {
      let first = stops[index].offset;
      let second = stops[index + 1].offset;
      let third = stops[index + 2].offset;

      if first.approx_eq(second) && second.approx_eq(third) {
        stops.remove(index + 1);
      } else {
        index += 1;
      }
    }
  }

  // Two stops at zero become zero and a hair above it.
  if stops.len() >= 2 {
    for index in 0..stops.len() - 1 {
      let first = stops[index].offset;
      let second = stops[index + 1].offset;

      if first.approx_eq(0.0) && second.approx_eq(0.0) {
        stops[index + 1].offset = (first + f32::EPSILON).clamp(0.0, 1.0);
      }
    }
  }

  // An offset below the one before it takes the previous one, which moves back a hair.
  for index in 1..stops.len() {
    let previous = stops[index - 1].offset;
    let current = stops[index].offset;

    if previous > current || previous.approx_eq(current) {
      stops[index - 1].offset = (previous - f32::EPSILON).clamp(0.0, 1.0);
      stops[index].offset = previous.clamp(0.0, 1.0);
    }
  }
}
