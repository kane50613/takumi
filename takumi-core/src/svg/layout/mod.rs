//! The render tree an SVG document lays out to, after Blink's `LayoutSVG*` objects: groups and
//! shapes with their transforms, clips, masks, and paint resolved.

use std::{
  collections::{HashMap, HashSet},
  hash::{Hash, Hasher},
  sync::Arc,
};

use tiny_skia_path::PathBuilder;

use self::paint_server::process_paint;
use crate::{
  fonts::font_fallback_map::FontFallbackMap,
  resources::font::Fonts,
  style::{
    BlendMode as StyleBlendMode, Color as StyleColor, ColorInput, Isolation,
    Visibility as StyleVisibility,
  },
  svg::tree::*,
  svg::{
    Attr, Display, Document, Length, LengthUnit as Unit, Node as SvgNode, PaintOrderPart,
    PreserveAspectRatio, Tag, ViewBox as ViewBoxAttr,
  },
};

mod clip_path;
mod filter;
mod image;
mod marker;
mod mask;
mod paint;
mod paint_server;
mod shape;
mod switch;
mod text;
mod units;
mod use_element;
mod value;

/// The size a percentage stands for when the root has neither a `viewBox` nor an absolute size.
const DEFAULT_SIZE: f32 = 100.0;

pub(crate) use filter::filters_from_markup;

pub use image::ImageHrefResolver;

#[derive(Clone)]
pub(crate) struct State<'a> {
  pub(crate) parent_clip_path: Option<SvgNode<'a>>,
  pub(crate) parent_markers: Vec<SvgNode<'a>>,
  /// Stores the resolved fill and stroke of a use node
  /// or a path element (for markers)
  pub(crate) context_element: Option<(Option<Fill>, Option<Stroke>)>,
  pub(crate) fe_image_link: bool,
  /// A viewBox of the parent SVG element.
  pub(crate) view_box: NonZeroRect,
  /// A size of the parent `use` element.
  /// Used only during nested `svg` size resolving.
  /// Width and height can be set independently.
  pub(crate) use_size: (Option<f32>, Option<f32>),
  pub(crate) opt: &'a Options<'a>,
}

/// What a generated resource id is named after.
#[derive(Clone, Copy)]
pub(crate) enum IdKind {
  LinearGradient,
  RadialGradient,
  Pattern,
  ClipPath,
  Mask,
  Filter,
  Image,
}

impl IdKind {
  fn prefix(self) -> &'static str {
    match self {
      Self::LinearGradient => "linearGradient",
      Self::RadialGradient => "radialGradient",
      Self::Pattern => "pattern",
      Self::ClipPath => "clipPath",
      Self::Mask => "mask",
      Self::Filter => "filter",
      Self::Image => "image",
    }
  }
}

pub(crate) struct Cache {
  pub(crate) fonts: Fonts,
  /// The fallback lists of every `<text>` in the tree, shared across them.
  pub(crate) font_fallback_map: FontFallbackMap,

  pub clip_paths: HashMap<String, Arc<ClipPath>>,
  pub masks: HashMap<String, Arc<Mask>>,
  pub filters: HashMap<String, Arc<crate::svg::tree::filter::Filter>>,
  pub paint: HashMap<String, Paint>,

  // used for ID generation
  all_ids: HashSet<u64>,
  counters: [usize; 7],
}

impl Cache {
  pub(crate) fn new(fonts: Fonts) -> Self {
    Self {
      fonts,
      font_fallback_map: FontFallbackMap::default(),

      clip_paths: HashMap::new(),
      masks: HashMap::new(),
      filters: HashMap::new(),
      paint: HashMap::new(),

      all_ids: HashSet::new(),
      counters: [0; 7],
    }
  }

  /// A resource id that no element of the document uses.
  pub(crate) fn gen_id(&mut self, kind: IdKind) -> NonEmptyString {
    loop {
      self.counters[kind as usize] += 1;

      let new_id = format!("{}{}", kind.prefix(), self.counters[kind as usize]);

      if !self.all_ids.contains(&string_hash(&new_id))
        && let Some(id) = NonEmptyString::new(new_id)
      {
        return id;
      }
    }
  }
}

// TODO: is there a simpler way?
fn string_hash(s: &str) -> u64 {
  let mut h = std::collections::hash_map::DefaultHasher::new();
  s.hash(&mut h);
  h.finish()
}

impl<'a> SvgNode<'a> {
  pub(crate) fn convert_length(
    &self,
    attr: Attr,
    object_units: Units,
    state: &State,
    def: Length,
  ) -> f32 {
    units::convert_length(
      self.parse(attr).unwrap_or(def),
      *self,
      attr,
      object_units,
      state,
    )
  }

  pub(crate) fn convert_user_length(&self, attr: Attr, state: &State, def: Length) -> f32 {
    self.convert_length(attr, Units::UserSpaceOnUse, state, def)
  }

  pub(crate) fn parse_viewbox(&self) -> Option<NonZeroRect> {
    let view_box: ViewBoxAttr = self.parse(Attr::ViewBox)?;

    NonZeroRect::from_xywh(view_box.x, view_box.y, view_box.width, view_box.height)
  }

  pub(crate) fn try_convert_length(
    &self,
    attr: Attr,
    object_units: Units,
    state: &State,
  ) -> Option<f32> {
    Some(units::convert_length(
      self.parse(attr)?,
      *self,
      attr,
      object_units,
      state,
    ))
  }

  pub(crate) fn is_visible_element(&self, opt: &Options) -> bool {
    self.style().is_some_and(|style| {
      style.display != Display::None && style.transform.is_none_or(|transform| transform.is_valid())
    }) && switch::is_condition_passed(*self, opt)
  }

  /// The element's `transform`, moved by its `transform-origin`.
  pub(crate) fn resolve_transform(&self, state: &State) -> Transform {
    let transform = self
      .style()
      .and_then(|style| style.transform)
      .unwrap_or_default();

    self.with_origin(transform, state)
  }

  /// A transform attribute such as `gradientTransform`, moved by the `transform-origin`.
  pub(crate) fn resolve_attribute_transform(&self, attr: Attr, state: &State) -> Transform {
    self.with_origin(self.parse(attr).unwrap_or_default(), state)
  }

  fn with_origin(&self, transform: Transform, state: &State) -> Transform {
    match self.style().and_then(|style| style.transform_origin) {
      Some(origin) => {
        let dx = units::convert_length(origin.x, *self, Attr::Width, Units::UserSpaceOnUse, state);
        let dy = units::convert_length(origin.y, *self, Attr::Height, Units::UserSpaceOnUse, state);

        Transform::default()
          .pre_translate(dx, dy)
          .pre_concat(transform)
          .pre_translate(-dx, -dy)
      }
      None => transform,
    }
  }
}

/// Splits a color into its opaque color and its alpha.
pub(crate) fn split_alpha(color: StyleColor) -> (Color, Opacity) {
  let [red, green, blue, alpha] = color.0;

  (Color::new_rgb(red, green, blue), Opacity::new_u8(alpha))
}

/// Resolves `currentColor` for `node`: its `color`, then `Options::current_color`, then black.
/// Falling through to the option marks `Options::current_color_used`.
pub(crate) fn resolve_current_color(node: SvgNode, state: &State) -> StyleColor {
  match node.style().and_then(|style| style.color) {
    Some(color) => color,
    None => {
      state
        .opt
        .current_color_used
        .store(true, std::sync::atomic::Ordering::Relaxed);
      state
        .opt
        .current_color
        .unwrap_or(StyleColor([0, 0, 0, 255]))
    }
  }
}

/// Converts an input `Document` into a `Tree`.
///
/// # Errors
///
/// - If `Document` doesn't have an SVG node - returns an empty tree.
/// - If `Document` doesn't have a valid size - returns `Error::InvalidSize`.
pub(crate) fn convert_doc(svg_doc: &Document, opt: &Options) -> Result<Tree, Error> {
  let svg = svg_doc.root_element();
  let (size, restore_viewbox) = resolve_svg_size(&svg, opt);
  let size = size?;
  let view_box = ViewBox {
    rect: svg
      .parse_viewbox()
      .unwrap_or_else(|| size.to_non_zero_rect(0.0, 0.0)),
    aspect: svg
      .parse::<PreserveAspectRatio>(Attr::PreserveAspectRatio)
      .unwrap_or_default(),
  };

  let background_color = match svg.style().map(|style| style.background_color) {
    Some(ColorInput::Value(color)) if color.0[3] != 0 => Some(color),
    _ => None,
  };

  let mut tree = Tree {
    size,
    root: Group::empty(),
    linear_gradients: Vec::new(),
    radial_gradients: Vec::new(),
    patterns: Vec::new(),
    clip_paths: Vec::new(),
    masks: Vec::new(),
    filters: Vec::new(),
  };

  if !svg.is_visible_element(opt) {
    return Ok(tree);
  }

  let state = State {
    parent_clip_path: None,
    context_element: None,
    parent_markers: Vec::new(),
    fe_image_link: false,
    view_box: view_box.rect,
    use_size: (None, None),
    opt,
  };

  let mut cache = Cache::new(opt.fonts.clone());

  for node in svg_doc.descendants() {
    if let Some(tag) = node.tag()
      && matches!(
        tag,
        Tag::ClipPath
          | Tag::Filter
          | Tag::LinearGradient
          | Tag::Mask
          | Tag::Pattern
          | Tag::RadialGradient
          | Tag::Image
      )
      && !node.element_id().is_empty()
    {
      cache.all_ids.insert(string_hash(node.element_id()));
    }
  }

  let root_ts = view_box.to_transform(tree.size());
  if root_ts.is_identity() && background_color.is_none() {
    convert_children(svg_doc.root(), &state, &mut cache, &mut tree.root);
  } else {
    let mut g = Group::empty();

    if let Some(background_color) = background_color
      && let Some(path) = background_path(background_color, view_box.rect.to_rect())
    {
      g.children.push(Node::Path(Box::new(path)));
    }

    g.transform = root_ts;
    g.abs_transform = root_ts;
    convert_children(svg_doc.root(), &state, &mut cache, &mut g);
    g.calculate_bounding_boxes();
    tree.root.children.push(Node::Group(Box::new(g)));
  }

  // Clear cache to make sure that all `Arc<T>` objects have a single strong reference.
  cache.clip_paths.clear();
  cache.masks.clear();
  cache.filters.clear();
  cache.paint.clear();

  paint_server::update_paint_servers(&mut tree.root, Transform::default(), None, None, &mut cache);
  tree.collect_paint_servers();
  tree.root.collect_clip_paths(&mut tree.clip_paths);
  tree.root.collect_masks(&mut tree.masks);
  tree.root.collect_filters(&mut tree.filters);
  tree.root.calculate_bounding_boxes();

  if restore_viewbox {
    calculate_svg_bbox(&mut tree);
  }

  Ok(tree)
}

fn background_path(background_color: StyleColor, area: Rect) -> Option<Path> {
  let path = PathBuilder::from_rect(area);
  let (color, opacity) = split_alpha(background_color);

  let fill = Fill {
    paint: Paint::Color(color),
    opacity,
    ..Default::default()
  };

  let mut path = Path::new_simple(Arc::new(path))?;
  path.fill = Some(fill);

  Some(path)
}

fn resolve_svg_size(svg: &SvgNode, opt: &Options) -> (Result<Size, Error>, bool) {
  let Some(view_box) = NonZeroRect::from_xywh(0.0, 0.0, 100.0, 100.0) else {
    return (Err(Error::InvalidSize), false);
  };
  let mut state = State {
    parent_clip_path: None,
    context_element: None,
    parent_markers: Vec::new(),
    fe_image_link: false,
    view_box,
    use_size: (None, None),
    opt,
  };

  let def = Length::new(100.0, Unit::Percent);
  let mut width: Length = svg.parse(Attr::Width).unwrap_or(def);
  let mut height: Length = svg.parse(Attr::Height).unwrap_or(def);

  let view_box = svg.parse_viewbox();

  let restore_viewbox =
    if (width.unit == Unit::Percent || height.unit == Unit::Percent) && view_box.is_none() {
      // Apply the percentages to the fallback size.
      if width.unit == Unit::Percent {
        width = Length::new((width.number / 100.0) * DEFAULT_SIZE, Unit::None);
      }

      if height.unit == Unit::Percent {
        height = Length::new((height.number / 100.0) * DEFAULT_SIZE, Unit::None);
      }

      true
    } else {
      false
    };

  let size = if let Some(vbox) = view_box {
    state.view_box = vbox;

    let w = if width.unit == Unit::Percent {
      vbox.width() * (width.number / 100.0)
    } else {
      svg.convert_user_length(Attr::Width, &state, def)
    };

    let h = if height.unit == Unit::Percent {
      vbox.height() * (height.number / 100.0)
    } else {
      svg.convert_user_length(Attr::Height, &state, def)
    };

    // If exactly one of width and height is specified, compute the missing
    // dimension from the specified one and the viewBox aspect ratio.
    match (
      svg.parse::<Length>(Attr::Width),
      svg.parse::<Length>(Attr::Height),
    ) {
      (Some(_), None) => Size::from_wh(w, vbox.height() * w / vbox.width()),
      (None, Some(_)) => Size::from_wh(vbox.width() * h / vbox.height(), h),
      (_, _) => Size::from_wh(w, h),
    }
  } else {
    Size::from_wh(
      svg.convert_user_length(Attr::Width, &state, def),
      svg.convert_user_length(Attr::Height, &state, def),
    )
  };

  (size.ok_or(Error::InvalidSize), restore_viewbox)
}

/// Calculates SVG's size and viewBox in case there were not set.
///
/// Simply iterates over all nodes and calculates a bounding box.
fn calculate_svg_bbox(tree: &mut Tree) {
  let bbox = tree.root.abs_bounding_box();
  if let Some(size) = Size::from_wh(bbox.right(), bbox.bottom()) {
    tree.size = size;
  }
}

#[inline(never)]
pub(crate) fn convert_children(
  parent_node: SvgNode,
  state: &State,
  cache: &mut Cache,
  parent: &mut Group,
) {
  for node in parent_node.children() {
    convert_element(node, state, cache, parent);
  }
}

#[inline(never)]
pub(crate) fn convert_element(node: SvgNode, state: &State, cache: &mut Cache, parent: &mut Group) {
  let tag_name = match node.tag() {
    Some(v) => v,
    None => return,
  };

  if !tag_name.is_graphic() && !matches!(tag_name, Tag::G | Tag::Switch | Tag::Svg) {
    return;
  }

  if !node.is_visible_element(state.opt) {
    return;
  }

  if tag_name == Tag::Use {
    use_element::convert(node, state, cache, parent);
    return;
  }

  if tag_name == Tag::Switch {
    switch::convert(node, state, cache, parent);
    return;
  }

  // A nested `svg` is handled just like `use`: it must not be wrapped in an
  // additional `convert_group`, otherwise its `transform` (and other group
  // properties) would be applied twice, since `convert_svg`
  // already creates its own group. The outermost `svg` (no parent element)
  // is intentionally not handled here - it's processed directly in
  // `convert_doc`.
  if tag_name == Tag::Svg && node.parent_element().is_some() {
    use_element::convert_svg(node, state, cache, parent);

    return;
  }

  if let Some(g) = convert_group(node, state, false, cache, parent, &|cache, g| {
    convert_element_impl(tag_name, node, state, cache, g);
  }) {
    parent.children.push(Node::Group(Box::new(g)));
  }
}

#[inline(never)]
fn convert_element_impl(
  tag_name: Tag,
  node: SvgNode,
  state: &State,
  cache: &mut Cache,
  parent: &mut Group,
) {
  match tag_name {
    Tag::Rect
    | Tag::Circle
    | Tag::Ellipse
    | Tag::Line
    | Tag::Polyline
    | Tag::Polygon
    | Tag::Path => {
      if let Some(path) = shape::convert(node, state) {
        convert_path(node, path, state, cache, parent);
      }
    }
    Tag::Image => {
      image::convert(node, state, cache, parent);
    }
    Tag::Text => {
      text::convert(node, state, cache, parent);
    }
    Tag::Svg => {
      // Only the outermost `svg` reaches this point; nested `svg` elements are
      // handled earlier in `convert_element`. The root `svg` itself is
      // skipped and only its children are converted.
      convert_children(node, state, cache, parent);
    }
    Tag::G => {
      convert_children(node, state, cache, parent);
    }
    _ => {}
  }
}

// `clipPath` can have only shape and `text` children.
//
// `line` doesn't impact rendering because stroke is always disabled
// for `clipPath` children.
#[inline(never)]
pub(crate) fn convert_clip_path_elements(
  clip_node: SvgNode,
  state: &State,
  cache: &mut Cache,
  parent: &mut Group,
) {
  for node in clip_node.children() {
    let tag_name = match node.tag() {
      Some(v) => v,
      None => continue,
    };

    if !tag_name.is_graphic() {
      continue;
    }

    if !node.is_visible_element(state.opt) {
      continue;
    }

    if tag_name == Tag::Use {
      use_element::convert(node, state, cache, parent);
      continue;
    }

    if let Some(g) = convert_group(node, state, false, cache, parent, &|cache, g| {
      convert_clip_path_elements_impl(tag_name, node, state, cache, g);
    }) {
      parent.children.push(Node::Group(Box::new(g)));
    }
  }
}

#[inline(never)]
fn convert_clip_path_elements_impl(
  tag_name: Tag,
  node: SvgNode,
  state: &State,
  cache: &mut Cache,
  parent: &mut Group,
) {
  match tag_name {
    Tag::Rect | Tag::Circle | Tag::Ellipse | Tag::Polyline | Tag::Polygon | Tag::Path => {
      if let Some(path) = shape::convert(node, state) {
        convert_path(node, path, state, cache, parent);
      }
    }
    Tag::Text => {
      text::convert(node, state, cache, parent);
    }
    _ => {
      log::warn!("'{}' is no a valid 'clip-path' child.", tag_name.name());
    }
  }
}

// TODO: explain
pub(crate) fn convert_group(
  node: SvgNode,
  state: &State,
  force: bool,
  cache: &mut Cache,
  parent: &mut Group,
  collect_children: &dyn Fn(&mut Cache, &mut Group),
) -> Option<Group> {
  let style = node.style()?;

  // A `clipPath` child cannot have an opacity.
  let opacity = if state.parent_clip_path.is_none() {
    Opacity::new_clamped(style.opacity)
  } else {
    Opacity::ONE
  };

  let transform = node.resolve_transform(state);
  let blend_mode = usvg_blend_mode(style.mix_blend_mode);
  let isolate = style.isolation == Isolation::Isolate;

  // Nodes generated by markers must not have an ID. Otherwise we would have duplicates.
  let is_g_or_use = matches!(node.tag(), Some(Tag::G) | Some(Tag::Use));
  let id = if is_g_or_use && state.parent_markers.is_empty() {
    node.element_id().to_string()
  } else {
    String::new()
  };

  let abs_transform = parent.abs_transform.pre_concat(transform);
  let mut g = Group {
    id,
    transform,
    abs_transform,
    opacity,
    blend_mode,
    isolate,
    ..Group::empty()
  };
  collect_children(cache, &mut g);

  // We need to know group's bounding box before converting
  // clipPaths, masks and filters.
  let object_bbox = g.calculate_object_bbox();

  // `mask` and `filter` cannot be set on `clipPath` children.
  // But `clip-path` can.

  let mut clip_path = None;
  if let Some(link) = node.reference(&style.clip_path) {
    clip_path = clip_path::convert(link, state, object_bbox, cache);
    clip_path.as_ref()?;
  }

  let mut mask = None;
  if state.parent_clip_path.is_none()
    && let Some(link) = node.reference(&style.mask)
  {
    mask = mask::convert(link, state, object_bbox, cache);
    mask.as_ref()?;
  }

  let filters = {
    let mut filters = Vec::new();
    if state.parent_clip_path.is_none() && !style.filter.is_empty() {
      if let Ok(f) = filter::convert(node, state, object_bbox, cache) {
        filters = f;
      } else {
        // A filter that not a link or a filter with a link to a non existing element.
        //
        // Unlike `clip-path` and `mask`, when a `filter` link is invalid
        // then the whole element should be ignored.
        //
        // This is kinda an undefined behaviour.
        // In most cases, Chrome, Firefox and rsvg will ignore such elements,
        // but in some cases Chrome allows it. Not sure why.
        // Inkscape (0.92) simply ignores such attributes, rendering element as is.
        // Batik (1.12) crashes.
        //
        // Test file: e-filter-051.svg
        return None;
      }
    }

    filters
  };

  let required = opacity.get().approx_ne_ulps(&1.0, 4)
    || clip_path.is_some()
    || mask.is_some()
    || !filters.is_empty()
    || !transform.is_identity()
    || blend_mode != BlendMode::Normal
    || isolate
    || is_g_or_use
    || force;

  if !required {
    parent.children.append(&mut g.children);
    return None;
  }

  g.clip_path = clip_path;
  g.mask = mask;
  g.filters = filters;

  // Must be called after we set Group::filters
  g.calculate_bounding_boxes();

  Some(g)
}

fn convert_path(
  node: SvgNode,
  tiny_skia_path: Arc<tiny_skia_path::Path>,
  state: &State,
  cache: &mut Cache,
  parent: &mut Group,
) {
  debug_assert!(tiny_skia_path.len() >= 2);
  if tiny_skia_path.len() < 2 {
    return;
  }

  let has_bbox = tiny_skia_path.bounds().width() > 0.0 && tiny_skia_path.bounds().height() > 0.0;
  let mut fill = paint::resolve_fill(node, has_bbox, state, cache);
  let mut stroke = paint::resolve_stroke(node, has_bbox, state, cache);
  let Some(style) = node.style() else {
    return;
  };
  let visibility = style.visibility == StyleVisibility::Visible;
  let mut visible = visibility;
  let rendering_mode = match style.shape_rendering {
    crate::svg::ShapeRendering::GeometricPrecision => ShapeRendering::GeometricPrecision,
    crate::svg::ShapeRendering::CrispEdges => ShapeRendering::CrispEdges,
    crate::svg::ShapeRendering::OptimizeSpeed => ShapeRendering::OptimizeSpeed,
  };

  // TODO: handle `markers` before `stroke`
  let raw_paint_order = style.paint_order;
  let paint_order = svg_paint_order_to_usvg(raw_paint_order);
  let path_transform = parent.abs_transform;

  // If a path doesn't have a fill or a stroke then it's invisible.
  // By setting `visibility` to `hidden` we are disabling rendering of this path.
  if fill.is_none() && stroke.is_none() {
    visible = false;
  }

  if let Some(fill) = fill.as_mut()
    && let Some(ContextElement::PathNode(context_transform, context_bbox)) = fill.context_element
  {
    process_paint(
      &mut fill.paint,
      true,
      context_transform,
      context_bbox.map(|r| r.to_rect()),
      path_transform,
      tiny_skia_path.bounds(),
      cache,
    );
    fill.context_element = None;
  }

  if let Some(stroke) = stroke.as_mut()
    && let Some(ContextElement::PathNode(context_transform, context_bbox)) = stroke.context_element
  {
    process_paint(
      &mut stroke.paint,
      true,
      context_transform,
      context_bbox.map(|r| r.to_rect()),
      path_transform,
      tiny_skia_path.bounds(),
      cache,
    );
    stroke.context_element = None;
  }

  let mut marker = None;
  if marker::is_valid(node) && visibility {
    let mut marker_group = Group {
      abs_transform: parent.abs_transform,
      ..Group::empty()
    };

    let mut marker_state = state.clone();

    let bbox = tiny_skia_path
      .compute_tight_bounds()
      .and_then(|r| r.to_non_zero_rect());

    let fill = fill.clone().map(|mut f| {
      f.context_element = Some(ContextElement::PathNode(path_transform, bbox));
      f
    });

    let stroke = stroke.clone().map(|mut s| {
      s.context_element = Some(ContextElement::PathNode(path_transform, bbox));
      s
    });

    marker_state.context_element = Some((fill, stroke));

    marker::convert(
      node,
      &tiny_skia_path,
      &marker_state,
      cache,
      &mut marker_group,
    );
    marker_group.calculate_bounding_boxes();
    marker = Some(marker_group);
  }

  // Nodes generated by markers must not have an ID. Otherwise we would have duplicates.
  let id = if state.parent_markers.is_empty() {
    node.element_id().to_string()
  } else {
    String::new()
  };

  let path = Path::new(
    id,
    visible,
    fill,
    stroke,
    paint_order,
    rendering_mode,
    tiny_skia_path,
    path_transform,
  );

  let path = match path {
    Some(v) => v,
    None => return,
  };

  match (raw_paint_order.0, marker) {
    ([PaintOrderPart::Markers, _, _], Some(markers_node)) => {
      parent.children.push(Node::Group(Box::new(markers_node)));
      parent.children.push(Node::Path(Box::new(path.clone())));
    }
    ([first, PaintOrderPart::Markers, last], Some(markers_node)) => {
      append_single_paint_path(first, &path, parent);
      parent.children.push(Node::Group(Box::new(markers_node)));
      append_single_paint_path(last, &path, parent);
    }
    ([_, _, PaintOrderPart::Markers], Some(markers_node)) => {
      parent.children.push(Node::Path(Box::new(path.clone())));
      parent.children.push(Node::Group(Box::new(markers_node)));
    }
    _ => parent.children.push(Node::Path(Box::new(path.clone()))),
  }
}

fn append_single_paint_path(paint_order_kind: PaintOrderPart, path: &Path, parent: &mut Group) {
  match paint_order_kind {
    PaintOrderPart::Fill => {
      if path.fill.is_some() {
        let mut fill_path = path.clone();
        fill_path.stroke = None;
        fill_path.id = String::new();
        parent.children.push(Node::Path(Box::new(fill_path)));
      }
    }
    PaintOrderPart::Stroke if path.stroke.is_some() => {
      let mut stroke_path = path.clone();
      stroke_path.fill = None;
      stroke_path.id = String::new();
      parent.children.push(Node::Path(Box::new(stroke_path)));
    }
    _ => {}
  }
}

pub(crate) fn svg_paint_order_to_usvg(order: crate::svg::PaintOrder) -> PaintOrder {
  match (order.0[0], order.0[1]) {
    (PaintOrderPart::Stroke, _) => PaintOrder::StrokeAndFill,
    (PaintOrderPart::Markers, PaintOrderPart::Stroke) => PaintOrder::StrokeAndFill,
    _ => PaintOrder::FillAndStroke,
  }
}

/// The tree's blend mode for `mode`.
fn usvg_blend_mode(mode: StyleBlendMode) -> BlendMode {
  match mode {
    StyleBlendMode::Multiply => BlendMode::Multiply,
    StyleBlendMode::Screen => BlendMode::Screen,
    StyleBlendMode::Overlay => BlendMode::Overlay,
    StyleBlendMode::Darken => BlendMode::Darken,
    StyleBlendMode::Lighten => BlendMode::Lighten,
    StyleBlendMode::ColorDodge => BlendMode::ColorDodge,
    StyleBlendMode::ColorBurn => BlendMode::ColorBurn,
    StyleBlendMode::HardLight => BlendMode::HardLight,
    StyleBlendMode::SoftLight => BlendMode::SoftLight,
    StyleBlendMode::Difference => BlendMode::Difference,
    StyleBlendMode::Exclusion => BlendMode::Exclusion,
    StyleBlendMode::Hue => BlendMode::Hue,
    StyleBlendMode::Saturation => BlendMode::Saturation,
    StyleBlendMode::Color => BlendMode::Color,
    StyleBlendMode::Luminosity => BlendMode::Luminosity,
    StyleBlendMode::PlusLighter => BlendMode::PlusLighter,
    _ => BlendMode::Normal,
  }
}

/// The tree's image rendering mode for `rendering`.
fn usvg_image_rendering(rendering: crate::svg::ImageRendering) -> ImageRendering {
  match rendering {
    crate::svg::ImageRendering::OptimizeQuality => ImageRendering::OptimizeQuality,
    crate::svg::ImageRendering::OptimizeSpeed => ImageRendering::OptimizeSpeed,
    crate::svg::ImageRendering::Smooth => ImageRendering::Smooth,
    crate::svg::ImageRendering::HighQuality => ImageRendering::HighQuality,
    crate::svg::ImageRendering::CrispEdges => ImageRendering::CrispEdges,
    crate::svg::ImageRendering::Pixelated => ImageRendering::Pixelated,
  }
}
