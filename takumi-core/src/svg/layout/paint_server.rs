use std::sync::Arc;

use super::{Cache, IdKind, State, resolve_current_color, split_alpha};
use crate::{
  resvg::usvg::*,
  style::ColorInput,
  svg::{Attr, Length, LengthUnit as Unit, Node as SvgNode, PreserveAspectRatio, Tag},
};

pub(crate) enum ServerOrColor {
  Server(Paint),
  Color { color: Color, opacity: Opacity },
}

pub(crate) fn convert(node: SvgNode, state: &State, cache: &mut Cache) -> Option<ServerOrColor> {
  // Check for existing.
  if let Some(paint) = cache.paint.get(node.element_id()) {
    return Some(ServerOrColor::Server(paint.clone()));
  }

  let paint = match node.tag()? {
    Tag::LinearGradient => convert_linear(node, state),
    Tag::RadialGradient => convert_radial(node, state),
    Tag::Pattern => convert_pattern(node, state, cache),
    _ => unreachable!(),
  };

  if let Some(ServerOrColor::Server(paint)) = &paint {
    cache
      .paint
      .insert(node.element_id().to_string(), paint.clone());
  }

  paint
}

#[inline(never)]
fn convert_linear(node: SvgNode, state: &State) -> Option<ServerOrColor> {
  let id = NonEmptyString::new(node.element_id().to_string())?;

  let stops = convert_stops(find_gradient_with_stops(node)?, state);
  if stops.len() < 2 {
    return stops_to_color(&stops);
  }

  let units = convert_units(node, Attr::GradientUnits, Units::ObjectBoundingBox);
  let transform = node.resolve_attribute_transform(Attr::GradientTransform, state);

  let gradient = LinearGradient {
    x1: resolve_number(node, Attr::X1, units, state, Length::number(0.0)),
    y1: resolve_number(node, Attr::Y1, units, state, Length::number(0.0)),
    x2: resolve_number(
      node,
      Attr::X2,
      units,
      state,
      Length::new(100.0, Unit::Percent),
    ),
    y2: resolve_number(node, Attr::Y2, units, state, Length::number(0.0)),
    base: BaseGradient {
      id,
      units,
      transform,
      spread_method: convert_spread_method(node),
      stops,
    },
  };

  Some(ServerOrColor::Server(Paint::LinearGradient(Arc::new(
    gradient,
  ))))
}

#[inline(never)]
fn convert_radial(node: SvgNode, state: &State) -> Option<ServerOrColor> {
  let id = NonEmptyString::new(node.element_id().to_string())?;

  let stops = convert_stops(find_gradient_with_stops(node)?, state);
  if stops.len() < 2 {
    return stops_to_color(&stops);
  }

  let units = convert_units(node, Attr::GradientUnits, Units::ObjectBoundingBox);
  let r = resolve_number(
    node,
    Attr::R,
    units,
    state,
    Length::new(50.0, Unit::Percent),
  );
  let fr = resolve_number(node, Attr::Fr, units, state, Length::number(0.0));

  // 'A value of zero will cause the area to be painted as a single color
  // using the color and opacity of the last gradient stop.'
  //
  // https://www.w3.org/TR/SVG11/pservers.html#RadialGradientElementRAttribute
  if !r.is_valid_length() {
    let stop = stops.last()?;
    return Some(ServerOrColor::Color {
      color: stop.color,
      opacity: stop.opacity,
    });
  }

  let spread_method = convert_spread_method(node);
  let cx = resolve_number(
    node,
    Attr::Cx,
    units,
    state,
    Length::new(50.0, Unit::Percent),
  );
  let cy = resolve_number(
    node,
    Attr::Cy,
    units,
    state,
    Length::new(50.0, Unit::Percent),
  );
  let fx = resolve_number(node, Attr::Fx, units, state, Length::number(cx));
  let fy = resolve_number(node, Attr::Fy, units, state, Length::number(cy));
  let transform = node.resolve_attribute_transform(Attr::GradientTransform, state);

  let gradient = RadialGradient {
    cx,
    cy,
    r: PositiveF32::new(r)?,
    fx,
    fy,
    fr: PositiveF32::new(fr).unwrap_or(PositiveF32::ZERO),
    base: BaseGradient {
      id,
      units,
      transform,
      spread_method,
      stops,
    },
  };

  Some(ServerOrColor::Server(Paint::RadialGradient(Arc::new(
    gradient,
  ))))
}

#[inline(never)]
fn convert_pattern(node: SvgNode, state: &State, cache: &mut Cache) -> Option<ServerOrColor> {
  let node_with_children = find_pattern_with_children(node)?;

  let id = NonEmptyString::new(node.element_id().to_string())?;

  let view_box = {
    let n1 = resolve_attr(node, Attr::ViewBox);
    let n2 = resolve_attr(node, Attr::PreserveAspectRatio);
    n1.parse_viewbox().map(|vb| ViewBox {
      rect: vb,
      aspect: n2
        .parse::<PreserveAspectRatio>(Attr::PreserveAspectRatio)
        .unwrap_or_default(),
    })
  };

  let units = convert_units(node, Attr::PatternUnits, Units::ObjectBoundingBox);
  let content_units = convert_units(node, Attr::PatternContentUnits, Units::UserSpaceOnUse);

  let transform = node.resolve_attribute_transform(Attr::PatternTransform, state);

  let rect = NonZeroRect::from_xywh(
    resolve_number(node, Attr::X, units, state, Length::number(0.0)),
    resolve_number(node, Attr::Y, units, state, Length::number(0.0)),
    resolve_number(node, Attr::Width, units, state, Length::number(0.0)),
    resolve_number(node, Attr::Height, units, state, Length::number(0.0)),
  );
  let rect = rect.log_none(|| {
    log::warn!(
      "Pattern '{}' has an invalid size. Skipped.",
      node.element_id()
    );
  })?;

  let mut patt = Pattern {
    id,
    units,
    content_units,
    transform,
    rect,
    view_box,
    root: Group::empty(),
  };

  // We can apply viewbox transform only for user space coordinates.
  // Otherwise we need a bounding box, which is unknown at this point.
  if let Some(view_box) = view_box
    && patt.units == Units::UserSpaceOnUse
    && patt.content_units == Units::UserSpaceOnUse
  {
    let mut g = Group::empty();
    g.transform = view_box.to_transform(rect.size());
    g.abs_transform = g.transform;

    super::convert_children(node_with_children, state, cache, &mut g);
    if !g.has_children() {
      return None;
    }

    g.calculate_bounding_boxes();
    patt.root.children.push(Node::Group(Box::new(g)));
  } else {
    super::convert_children(node_with_children, state, cache, &mut patt.root);
    if !patt.root.has_children() {
      return None;
    }
  }

  patt.root.calculate_bounding_boxes();

  Some(ServerOrColor::Server(Paint::Pattern(Arc::new(patt))))
}

fn convert_spread_method(node: SvgNode) -> SpreadMethod {
  let node = resolve_attr(node, Attr::SpreadMethod);
  node.parse(Attr::SpreadMethod).unwrap_or_default()
}

pub(crate) fn convert_units(node: SvgNode, name: Attr, def: Units) -> Units {
  let node = resolve_attr(node, name);
  node.parse(name).unwrap_or(def)
}

fn find_gradient_with_stops<'a>(node: SvgNode<'a>) -> Option<SvgNode<'a>> {
  for link in node.href_chain() {
    if !link.tag()?.is_gradient() {
      log::warn!(
        "Gradient '{}' cannot reference '{}' via 'xlink:href'.",
        node.element_id(),
        link.tag().map_or("", Tag::name)
      );
      return None;
    }

    if link.children().any(|n| n.tag() == Some(Tag::Stop)) {
      return Some(link);
    }
  }

  None
}

fn find_pattern_with_children<'a>(node: SvgNode<'a>) -> Option<SvgNode<'a>> {
  for link in node.href_chain() {
    if link.tag() != Some(Tag::Pattern) {
      log::warn!(
        "Pattern '{}' cannot reference '{}' via 'xlink:href'.",
        node.element_id(),
        link.tag().map_or("", Tag::name)
      );
      return None;
    }

    if link.has_children() {
      return Some(link);
    }
  }

  None
}

fn convert_stops(grad: SvgNode, state: &State) -> Vec<Stop> {
  let mut stops = Vec::new();

  {
    let mut prev_offset = Length::number(0.0);
    for stop in grad.children() {
      if stop.tag() != Some(Tag::Stop) {
        log::warn!("Invalid gradient child: '{:?}'.", stop.tag());
        continue;
      }

      // `number` can be either a number or a percentage.
      let offset = stop.parse(Attr::Offset).unwrap_or(prev_offset);
      let offset = match offset.unit {
        Unit::None => offset.number,
        Unit::Percent => offset.number / 100.0,
        _ => prev_offset.number,
      };
      prev_offset = Length::number(offset);
      let offset = crate::resvg::usvg::f32_bound(0.0, offset, 1.0);

      let (stop_color, stop_opacity) = stop.style().map_or((ColorInput::default(), 1.0), |style| {
        (style.stop_color, style.stop_opacity)
      });
      let (color, opacity) = split_alpha(match stop_color {
        ColorInput::Value(color) => color,
        _ => resolve_current_color(stop, state),
      });
      let stop_opacity = Opacity::new_clamped(stop_opacity);
      stops.push(Stop {
        offset: StopOffset::new_clamped(offset),
        color,
        opacity: opacity * stop_opacity,
      });
    }
  }

  // Remove stops with equal offset.
  //
  // Example:
  // offset="0.5"
  // offset="0.7"
  // offset="0.7" <-- this one should be removed
  // offset="0.7"
  // offset="0.9"
  if stops.len() >= 3 {
    let mut i = 0;
    while i < stops.len() - 2 {
      let offset1 = stops[i].offset.get();
      let offset2 = stops[i + 1].offset.get();
      let offset3 = stops[i + 2].offset.get();

      if offset1.approx_eq_ulps(&offset2, 4) && offset2.approx_eq_ulps(&offset3, 4) {
        // Remove offset in the middle.
        stops.remove(i + 1);
      } else {
        i += 1;
      }
    }
  }

  // Remove zeros.
  //
  // From:
  // offset="0.0"
  // offset="0.0"
  // offset="0.7"
  //
  // To:
  // offset="0.0"
  // offset="0.00000001"
  // offset="0.7"
  if stops.len() >= 2 {
    let mut i = 0;
    while i < stops.len() - 1 {
      let offset1 = stops[i].offset.get();
      let offset2 = stops[i + 1].offset.get();

      if offset1.approx_eq_ulps(&0.0, 4) && offset2.approx_eq_ulps(&0.0, 4) {
        stops[i + 1].offset = StopOffset::new_clamped(offset1 + f32::EPSILON);
      }

      i += 1;
    }
  }

  // Shift equal offsets.
  //
  // From:
  // offset="0.5"
  // offset="0.7"
  // offset="0.7"
  //
  // To:
  // offset="0.5"
  // offset="0.699999999"
  // offset="0.7"
  {
    let mut i = 1;
    while i < stops.len() {
      let offset1 = stops[i - 1].offset.get();
      let offset2 = stops[i].offset.get();

      // Next offset must be smaller then previous.
      if offset1 > offset2 || offset1.approx_eq_ulps(&offset2, 4) {
        // Make previous offset a bit smaller.
        let new_offset = offset1 - f32::EPSILON;
        stops[i - 1].offset = StopOffset::new_clamped(new_offset);
        stops[i].offset = StopOffset::new_clamped(offset1);
      }

      i += 1;
    }
  }

  stops
}

#[inline(never)]
pub(crate) fn resolve_number(
  node: SvgNode,
  name: Attr,
  units: Units,
  state: &State,
  def: Length,
) -> f32 {
  resolve_attr(node, name).convert_length(name, units, state, def)
}

fn resolve_attr<'a>(node: SvgNode<'a>, name: Attr) -> SvgNode<'a> {
  if node.has_attribute(name) {
    return node;
  }

  match node.tag() {
    Some(Tag::LinearGradient) => resolve_lg_attr(node, name),
    Some(Tag::RadialGradient) => resolve_rg_attr(node, name),
    Some(Tag::Pattern) => resolve_pattern_attr(node, name),
    Some(Tag::Filter) => resolve_filter_attr(node, name),
    _ => node,
  }
}

fn resolve_lg_attr<'a>(node: SvgNode<'a>, name: Attr) -> SvgNode<'a> {
  for link in node.href_chain() {
    let tag_name = match link.tag() {
      Some(v) => v,
      None => return node,
    };

    match (name, tag_name) {
            // Coordinates can be resolved only from
            // ref element with the same type.
              (Attr::X1, Tag::LinearGradient)
            | (Attr::Y1, Tag::LinearGradient)
            | (Attr::X2, Tag::LinearGradient)
            | (Attr::Y2, Tag::LinearGradient)
            // Other attributes can be resolved
            // from any kind of gradient.
            | (Attr::GradientUnits, Tag::LinearGradient)
            | (Attr::GradientUnits, Tag::RadialGradient)
            | (Attr::SpreadMethod, Tag::LinearGradient)
            | (Attr::SpreadMethod, Tag::RadialGradient)
            | (Attr::GradientTransform, Tag::LinearGradient)
            | (Attr::GradientTransform, Tag::RadialGradient) => {
                if link.has_attribute(name) {
                    return link;
                }
            }
            _ => break,
        }
  }

  node
}

fn resolve_rg_attr<'a>(node: SvgNode<'a>, name: Attr) -> SvgNode<'a> {
  for link in node.href_chain() {
    let tag_name = match link.tag() {
      Some(v) => v,
      None => return node,
    };

    match (name, tag_name) {
            // Coordinates can be resolved only from
            // ref element with the same type.
              (Attr::Cx, Tag::RadialGradient)
            | (Attr::Cy, Tag::RadialGradient)
            | (Attr::R,  Tag::RadialGradient)
            | (Attr::Fx, Tag::RadialGradient)
            | (Attr::Fy, Tag::RadialGradient)
            | (Attr::Fr, Tag::RadialGradient)
            // Other attributes can be resolved
            // from any kind of gradient.
            | (Attr::GradientUnits, Tag::LinearGradient)
            | (Attr::GradientUnits, Tag::RadialGradient)
            | (Attr::SpreadMethod, Tag::LinearGradient)
            | (Attr::SpreadMethod, Tag::RadialGradient)
            | (Attr::GradientTransform, Tag::LinearGradient)
            | (Attr::GradientTransform, Tag::RadialGradient) => {
                if link.has_attribute(name) {
                    return link;
                }
            }
            _ => break,
        }
  }

  node
}

fn resolve_pattern_attr<'a>(node: SvgNode<'a>, name: Attr) -> SvgNode<'a> {
  for link in node.href_chain() {
    let tag_name = match link.tag() {
      Some(v) => v,
      None => return node,
    };

    if tag_name != Tag::Pattern {
      break;
    }

    if link.has_attribute(name) {
      return link;
    }
  }

  node
}

fn resolve_filter_attr<'a>(node: SvgNode<'a>, aid: Attr) -> SvgNode<'a> {
  for link in node.href_chain() {
    let tag_name = match link.tag() {
      Some(v) => v,
      None => return node,
    };

    if tag_name != Tag::Filter {
      break;
    }

    if link.has_attribute(aid) {
      return link;
    }
  }

  node
}

fn stops_to_color(stops: &[Stop]) -> Option<ServerOrColor> {
  if stops.is_empty() {
    None
  } else {
    Some(ServerOrColor::Color {
      color: stops[0].color,
      opacity: stops[0].opacity,
    })
  }
}

// Update paints servers by doing the following:
// 1. Replace context fills/strokes that are linked to
// a use node with their actual values.
// 2. Convert all object units to UserSpaceOnUse
pub fn update_paint_servers(
  group: &mut Group,
  context_transform: Transform,
  context_bbox: Option<Rect>,
  text_bbox: Option<Rect>,
  cache: &mut Cache,
) {
  for child in &mut group.children {
    // Set context transform and bbox if applicable if the
    // current group is a use node.
    let (context_transform, context_bbox) = if group.is_context_element {
      (group.abs_transform, Some(group.bounding_box))
    } else {
      (context_transform, context_bbox)
    };

    node_to_user_coordinates(child, context_transform, context_bbox, text_bbox, cache);
  }
}

// When parsing clipPaths, masks and filters we already know group's bounding box.
// But with gradients and patterns we don't, because we have to know text bounding box
// before we even parsed it. Which is impossible.
// Therefore our only choice is to parse gradients and patterns preserving their units
// and then replace them with `userSpaceOnUse` after the whole tree parsing is finished.
// So while gradients and patterns do still store their units,
// they are not exposed in the public API and for the caller they are always `userSpaceOnUse`.
fn node_to_user_coordinates(
  node: &mut Node,
  context_transform: Transform,
  context_bbox: Option<Rect>,
  text_bbox: Option<Rect>,
  cache: &mut Cache,
) {
  match node {
    Node::Group(g) => {
      // No need to check clip paths, because they cannot have paint servers.
      if let Some(mask) = &mut g.mask
        && let Some(mask) = Arc::get_mut(mask)
      {
        update_paint_servers(&mut mask.root, context_transform, context_bbox, None, cache);

        if let Some(sub_mask) = &mut mask.mask
          && let Some(sub_mask) = Arc::get_mut(sub_mask)
        {
          update_paint_servers(
            &mut sub_mask.root,
            context_transform,
            context_bbox,
            None,
            cache,
          );
        }
      }

      for filter in &mut g.filters {
        if let Some(filter) = Arc::get_mut(filter) {
          for primitive in &mut filter.primitives {
            if let filter::Kind::Image(image) = &mut primitive.kind {
              update_paint_servers(
                &mut image.root,
                context_transform,
                context_bbox,
                None,
                cache,
              );
            }
          }
        }
      }

      update_paint_servers(g, context_transform, context_bbox, text_bbox, cache);
    }
    Node::Path(path) => {
      // Paths inside `Text::flattened` are special and must use text's bounding box
      // instead of their own.
      let bbox = text_bbox.unwrap_or(path.bounding_box);

      process_fill(
        &mut path.fill,
        path.abs_transform,
        context_transform,
        context_bbox,
        bbox,
        cache,
      );
      process_stroke(
        &mut path.stroke,
        path.abs_transform,
        context_transform,
        context_bbox,
        bbox,
        cache,
      );
    }
    Node::Image(image) => {
      if let ImageKind::SVG(tree) = &mut image.kind {
        update_paint_servers(&mut tree.root, context_transform, context_bbox, None, cache);
      }
    }
    Node::Text(text) => {
      // By the SVG spec, `tspan` doesn't have a bbox and uses the parent `text` bbox.
      // Therefore we have to use text's bbox when converting tspan and flatted text
      // paint servers.
      let bbox = text.bounding_box;

      // We need to update three things:
      // 1. The fills/strokes of the original elements in the usvg tree.
      // 2. The fills/strokes of the layouted elements of the text.
      // 3. The fills/strokes of the outlined text.

      // 1.
      for chunk in &mut text.chunks {
        for span in &mut chunk.spans {
          process_fill(
            &mut span.fill,
            text.abs_transform,
            context_transform,
            context_bbox,
            bbox,
            cache,
          );
          process_stroke(
            &mut span.stroke,
            text.abs_transform,
            context_transform,
            context_bbox,
            bbox,
            cache,
          );
          process_text_decoration(&mut span.decoration.underline, bbox, cache);
          process_text_decoration(&mut span.decoration.overline, bbox, cache);
          process_text_decoration(&mut span.decoration.line_through, bbox, cache);
        }
      }

      // 2.
      for span in &mut text.layouted {
        process_fill(
          &mut span.fill,
          text.abs_transform,
          context_transform,
          context_bbox,
          bbox,
          cache,
        );
        process_stroke(
          &mut span.stroke,
          text.abs_transform,
          context_transform,
          context_bbox,
          bbox,
          cache,
        );

        let mut process_decoration = |path: &mut Path| {
          process_fill(
            &mut path.fill,
            text.abs_transform,
            context_transform,
            context_bbox,
            bbox,
            cache,
          );
          process_stroke(
            &mut path.stroke,
            text.abs_transform,
            context_transform,
            context_bbox,
            bbox,
            cache,
          );
        };

        if let Some(path) = &mut span.overline {
          process_decoration(path);
        }

        if let Some(path) = &mut span.underline {
          process_decoration(path);
        }

        if let Some(path) = &mut span.line_through {
          process_decoration(path);
        }
      }

      // 3.
      update_paint_servers(
        &mut text.flattened,
        context_transform,
        context_bbox,
        Some(bbox),
        cache,
      );
    }
  }
}

fn process_fill(
  fill: &mut Option<Fill>,
  path_transform: Transform,
  context_transform: Transform,
  context_bbox: Option<Rect>,
  bbox: Rect,
  cache: &mut Cache,
) {
  let mut ok = false;
  if let Some(fill) = fill.as_mut() {
    // Path context elements (i.e. for  markers) have already been resolved,
    // so we only care about use nodes.
    ok = process_paint(
      &mut fill.paint,
      matches!(fill.context_element, Some(ContextElement::UseNode)),
      context_transform,
      context_bbox,
      path_transform,
      bbox,
      cache,
    );
  }
  if !ok {
    *fill = None;
  }
}

fn process_stroke(
  stroke: &mut Option<Stroke>,
  path_transform: Transform,
  context_transform: Transform,
  context_bbox: Option<Rect>,
  bbox: Rect,
  cache: &mut Cache,
) {
  let mut ok = false;
  if let Some(stroke) = stroke.as_mut() {
    // Path context elements (i.e. for  markers) have already been resolved,
    // so we only care about use nodes.
    ok = process_paint(
      &mut stroke.paint,
      matches!(stroke.context_element, Some(ContextElement::UseNode)),
      context_transform,
      context_bbox,
      path_transform,
      bbox,
      cache,
    );
  }
  if !ok {
    *stroke = None;
  }
}

fn process_context_paint(
  paint: &mut Paint,
  context_transform: Transform,
  path_transform: Transform,
  cache: &mut Cache,
) -> Option<()> {
  // The idea is the following: We have a certain context element that has
  // a transform A, and further below in the tree we have for example a path
  // whose paint has a transform C. In order to get from A to C, there is some
  // transformation matrix B such that A x B = C. We now need to figure out
  // a way to get from C back to A, so that the transformation of the paint
  // matches the one from the context element, even if B was applied. How
  // do we do that? We calculate CxB^(-1), which will overall then have
  // the same effect as A. How do we calculate B^(-1)?
  // --> (A^(-1)xC)^(-1)
  let rev_transform = context_transform
    .invert()?
    .pre_concat(path_transform)
    .invert()?;

  match paint {
    Paint::Color(_) => {}
    Paint::LinearGradient(lg) => {
      let transform = lg.transform.post_concat(rev_transform);
      *paint = Paint::LinearGradient(Arc::new(LinearGradient {
        x1: lg.x1,
        y1: lg.y1,
        x2: lg.x2,
        y2: lg.y2,
        base: BaseGradient {
          id: cache.gen_id(IdKind::LinearGradient),
          units: lg.units,
          transform,
          spread_method: lg.spread_method,
          stops: lg.stops.clone(),
        },
      }));
    }
    Paint::RadialGradient(rg) => {
      let transform = rg.transform.post_concat(rev_transform);
      *paint = Paint::RadialGradient(Arc::new(RadialGradient {
        cx: rg.cx,
        cy: rg.cy,
        r: rg.r,
        fx: rg.fx,
        fy: rg.fy,
        fr: rg.fr,
        base: BaseGradient {
          id: cache.gen_id(IdKind::RadialGradient),
          units: rg.units,
          transform,
          spread_method: rg.spread_method,
          stops: rg.stops.clone(),
        },
      }));
    }
    Paint::Pattern(pat) => {
      let transform = pat.transform.post_concat(rev_transform);
      *paint = Paint::Pattern(Arc::new(Pattern {
        id: cache.gen_id(IdKind::Pattern),
        units: pat.units,
        content_units: pat.content_units,
        transform,
        rect: pat.rect,
        view_box: pat.view_box,
        root: pat.root.clone(),
      }));
    }
  }

  Some(())
}

pub(crate) fn process_paint(
  paint: &mut Paint,
  has_context: bool,
  context_transform: Transform,
  context_bbox: Option<Rect>,
  path_transform: Transform,
  bbox: Rect,
  cache: &mut Cache,
) -> bool {
  if paint.units() == Units::ObjectBoundingBox || paint.content_units() == Units::ObjectBoundingBox
  {
    let bbox = if has_context {
      let Some(bbox) = context_bbox else {
        return false;
      };
      bbox
    } else {
      bbox
    };

    if paint.convert_to_user_space(bbox, cache).is_none() {
      return false;
    }
  }

  if let Paint::Pattern(patt) = paint
    && let Some(patt) = Arc::get_mut(patt)
  {
    update_paint_servers(&mut patt.root, Transform::default(), None, None, cache);
  }

  if has_context {
    process_context_paint(paint, context_transform, path_transform, cache);
  }

  true
}

fn process_text_decoration(style: &mut Option<TextDecorationStyle>, bbox: Rect, cache: &mut Cache) {
  if let Some(style) = style.as_mut() {
    process_fill(
      &mut style.fill,
      Transform::default(),
      Transform::default(),
      None,
      bbox,
      cache,
    );
    process_stroke(
      &mut style.stroke,
      Transform::default(),
      Transform::default(),
      None,
      bbox,
      cache,
    );
  }
}

impl Paint {
  fn convert_to_user_space(&mut self, bbox: Rect, cache: &mut Cache) -> Option<()> {
    let name = if matches!(self, Paint::Pattern(_)) {
      "Pattern"
    } else {
      "Gradient"
    };
    let bbox = bbox
      .to_non_zero_rect()
      .log_none(|| log::warn!("{} on zero-sized shapes is not allowed.", name))?;

    // `Arc::get_mut()` allow us to modify some paint servers in-place.
    // This reduces the amount of cloning and preserves the original ID as well.
    match self {
      Paint::Color(_) => {} // unreachable
      Paint::LinearGradient(lg) => {
        let transform = lg.transform.post_concat(Transform::from_bbox(bbox));
        if let Some(lg) = Arc::get_mut(lg) {
          lg.base.transform = transform;
          lg.base.units = Units::UserSpaceOnUse;
        } else {
          *lg = Arc::new(LinearGradient {
            x1: lg.x1,
            y1: lg.y1,
            x2: lg.x2,
            y2: lg.y2,
            base: BaseGradient {
              id: cache.gen_id(IdKind::LinearGradient),
              units: Units::UserSpaceOnUse,
              transform,
              spread_method: lg.spread_method,
              stops: lg.stops.clone(),
            },
          });
        }
      }
      Paint::RadialGradient(rg) => {
        let transform = rg.transform.post_concat(Transform::from_bbox(bbox));
        if let Some(rg) = Arc::get_mut(rg) {
          rg.base.transform = transform;
          rg.base.units = Units::UserSpaceOnUse;
        } else {
          *rg = Arc::new(RadialGradient {
            cx: rg.cx,
            cy: rg.cy,
            r: rg.r,
            fx: rg.fx,
            fy: rg.fy,
            fr: rg.fr,
            base: BaseGradient {
              id: cache.gen_id(IdKind::RadialGradient),
              units: Units::UserSpaceOnUse,
              transform,
              spread_method: rg.spread_method,
              stops: rg.stops.clone(),
            },
          });
        }
      }
      Paint::Pattern(patt) => {
        let rect = if patt.units == Units::ObjectBoundingBox {
          patt.rect.bbox_transform(bbox)
        } else {
          patt.rect
        };

        if let Some(patt) = Arc::get_mut(patt) {
          patt.rect = rect;
          patt.units = Units::UserSpaceOnUse;

          if patt.content_units == Units::ObjectBoundingBox && patt.view_box.is_none() {
            // No need to shift patterns.
            let transform = Transform::from_scale(bbox.width(), bbox.height());
            push_pattern_transform(&mut patt.root, transform);
          }

          if let Some(view_box) = patt.view_box {
            push_pattern_transform(&mut patt.root, view_box.to_transform(rect.size()));
          }

          patt.content_units = Units::UserSpaceOnUse;
        } else {
          let mut root =
            if patt.content_units == Units::ObjectBoundingBox && patt.view_box.is_none() {
              // No need to shift patterns.
              let transform = Transform::from_scale(bbox.width(), bbox.height());

              let mut g = patt.root.clone();
              push_pattern_transform(&mut g, transform);
              g
            } else {
              patt.root.clone()
            };

          if let Some(view_box) = patt.view_box {
            push_pattern_transform(&mut root, view_box.to_transform(rect.size()));
          }

          *patt = Arc::new(Pattern {
            id: cache.gen_id(IdKind::Pattern),
            units: Units::UserSpaceOnUse,
            content_units: Units::UserSpaceOnUse,
            transform: patt.transform,
            rect,
            view_box: patt.view_box,
            root,
          });
        }
      }
    }

    Some(())
  }
}

fn push_pattern_transform(root: &mut Group, transform: Transform) {
  // TODO: we should update abs_transform in all descendants as well
  let mut g = std::mem::replace(root, Group::empty());
  g.transform = transform;
  g.abs_transform = transform;

  root.children.push(Node::Group(Box::new(g)));
  root.calculate_bounding_boxes();
}

impl Paint {
  #[inline]
  pub(crate) fn units(&self) -> Units {
    match self {
      Self::Color(_) => Units::UserSpaceOnUse,
      Self::LinearGradient(lg) => lg.units,
      Self::RadialGradient(rg) => rg.units,
      Self::Pattern(patt) => patt.units,
    }
  }

  #[inline]
  pub(crate) fn content_units(&self) -> Units {
    match self {
      Self::Pattern(patt) => patt.content_units,
      _ => Units::UserSpaceOnUse,
    }
  }
}
