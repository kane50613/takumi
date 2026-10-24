//! A collection of SVG filters.

use std::collections::HashSet;
use std::sync::Arc;

use strict_num::PositiveF32;

use super::{
  Cache, IdKind, State, convert_element,
  image::{self, ImagePlacement},
  paint_server::{convert_units, resolve_number},
  resolve_current_color, split_alpha, units,
  value::FromValue,
};
use crate::{
  style::{ColorInput, Filter as StyleFilter, Length as StyleLength, TextShadow},
  svg::tree::{
    ApproxZeroUlps, Color, Error, Group, Node, NonEmptyString, NonZeroF32, NonZeroRect, Opacity,
    OptionLog, Options, Size, Units,
    filter::{self, *},
  },
  svg::{
    Attr, ColorInterpolation as StyleColorInterpolation, Document, FilterValue, Length, LengthUnit,
    Node as SvgNode, ParseOptions, PreserveAspectRatio, Scanner, StyleContext, Tag,
  },
};

impl FromValue<'_> for filter::ColorInterpolation {
  fn parse(_: SvgNode, _: Attr, value: &str) -> Option<Self> {
    match value {
      "sRGB" => Some(filter::ColorInterpolation::SRGB),
      "linearRGB" => Some(filter::ColorInterpolation::LinearRGB),
      _ => None,
    }
  }
}

/// Parses standalone `<filter>` markup (carrying `id="{filter_id}"`) and
/// resolves it against a `width` x `height` element box, skipping the render
/// tree entirely.
///
/// Returns `Ok(None)` when the filter reference is invalid, which per the
/// converter's behaviour hides the filtered element.
pub(crate) fn filters_from_markup(
  markup: &str,
  filter_id: &str,
  width: f32,
  height: f32,
  opt: &Options,
) -> Result<Option<Vec<Arc<Filter>>>, Error> {
  let document = format!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}">{markup}<rect width="{width}" height="{height}" filter="url(#{filter_id})"/></svg>"#
  );
  let doc = Document::parse(
    &document,
    &ParseOptions {
      style: StyleContext {
        font_size: opt.font_size,
        dpi: opt.dpi,
      },
      style_sheet: None,
    },
  )?;

  let bbox = NonZeroRect::from_xywh(0.0, 0.0, width, height).ok_or(Error::InvalidSize)?;
  let state = State {
    parent_clip_path: None,
    parent_markers: Vec::new(),
    context_element: None,
    fe_image_link: false,
    view_box: bbox,
    use_size: (None, None),
    opt,
  };
  let mut cache = Cache::new(opt.fonts.clone());
  let node = doc
    .descendants()
    .find(|node| node.style().is_some_and(|style| !style.filter.is_empty()))
    .ok_or(Error::InvalidSize)?;

  Ok(convert(node, &state, Some(bbox), &mut cache).ok())
}

pub(crate) fn convert(
  node: SvgNode,
  state: &State,
  object_bbox: Option<NonZeroRect>,
  cache: &mut Cache,
) -> Result<Vec<Arc<Filter>>, ()> {
  let Some(style) = node.style() else {
    return Ok(Vec::new());
  };

  let mut has_invalid_urls = false;
  let mut filters = Vec::new();

  let create_base_filter_func = |kind, filters: &mut Vec<Arc<Filter>>, cache: &mut Cache| {
    // Filter functions, unlike `filter` elements, do not have a filter region.
    // We're currently do not support an unlimited region, so we simply use a fairly large one.
    // This if far from ideal, but good for now.
    // TODO: Should be fixed eventually.
    let Some(mut rect) = (match kind {
      Kind::DropShadow(_) | Kind::GaussianBlur(_) => NonZeroRect::from_xywh(-0.5, -0.5, 2.0, 2.0),
      _ => NonZeroRect::from_xywh(-0.1, -0.1, 1.2, 1.2),
    }) else {
      return;
    };

    let object_bbox = match object_bbox {
      Some(v) => v,
      None => {
        log::warn!(
          "Filter '{}' has an invalid region. Skipped.",
          node.element_id()
        );
        return;
      }
    };

    rect = rect.bbox_transform(object_bbox);

    filters.push(Arc::new(Filter {
      id: cache.gen_id(IdKind::Filter),
      rect,
      primitives: vec![Primitive {
        rect,
        // Unlike `filter` elements, filter functions use sRGB colors by default.
        color_interpolation: ColorInterpolation::SRGB,
        result: "result".to_string(),
        kind,
      }],
    }));
  };

  for func in &style.filter {
    match func {
      FilterValue::Function(function) => {
        let kind = match function {
          StyleFilter::Blur(std_dev) => convert_blur_function(node, *std_dev, state),
          StyleFilter::DropShadow(shadow) => convert_drop_shadow_function(node, shadow, state),
          StyleFilter::Brightness(amount) => convert_brightness_function(amount.0),
          StyleFilter::Contrast(amount) => convert_contrast_function(amount.0),
          StyleFilter::Grayscale(amount) => convert_grayscale_function(amount.0),
          StyleFilter::HueRotate(angle) => convert_hue_rotate_function(**angle),
          StyleFilter::Invert(amount) => convert_invert_function(amount.0),
          StyleFilter::Opacity(amount) => convert_opacity_function(amount.0),
          StyleFilter::Sepia(amount) => convert_sepia_function(amount.0),
          StyleFilter::Saturate(amount) => convert_saturate_function(amount.0),
          _ => continue,
        };

        create_base_filter_func(kind, &mut filters, cache);
      }
      FilterValue::Url(url) => {
        if let Some(link) = node.document().element_by_id(url) {
          if let Ok(res) = convert_url(link, state, object_bbox, cache) {
            if let Some(f) = res {
              filters.push(f);
            }
          } else {
            has_invalid_urls = true;
          }
        } else {
          has_invalid_urls = true;
        }
      }
    }
  }

  // If a `filter` attribute had urls pointing to a missing elements
  // and there are no valid filters at all - this is an error.
  //
  // Note that an invalid url is not an error in general.
  if filters.is_empty() && has_invalid_urls {
    return Err(());
  }

  Ok(filters)
}

fn convert_url(
  node: SvgNode,
  state: &State,
  object_bbox: Option<NonZeroRect>,
  cache: &mut Cache,
) -> Result<Option<Arc<Filter>>, ()> {
  let units = convert_units(node, Attr::FilterUnits, Units::ObjectBoundingBox);
  let primitive_units = convert_units(node, Attr::PrimitiveUnits, Units::UserSpaceOnUse);

  // Check if this element was already converted.
  //
  // Only `userSpaceOnUse` clipPaths can be shared,
  // because `objectBoundingBox` one will be converted into user one
  // and will become node-specific.
  let cacheable = units == Units::UserSpaceOnUse && primitive_units == Units::UserSpaceOnUse;
  if cacheable && let Some(filter) = cache.filters.get(node.element_id()) {
    return Ok(Some(filter.clone()));
  }

  let rect = NonZeroRect::from_xywh(
    resolve_number(node, Attr::X, units, state, Length::percent(-10.0)),
    resolve_number(node, Attr::Y, units, state, Length::percent(-10.0)),
    resolve_number(node, Attr::Width, units, state, Length::percent(120.0)),
    resolve_number(node, Attr::Height, units, state, Length::percent(120.0)),
  );

  let mut rect = rect
    .log_none(|| {
      log::warn!(
        "Filter '{}' has an invalid region. Skipped.",
        node.element_id()
      );
    })
    .ok_or(())?;

  if units == Units::ObjectBoundingBox {
    if let Some(object_bbox) = object_bbox {
      rect = rect.bbox_transform(object_bbox);
    } else {
      log::warn!("Filters on zero-sized shapes are not allowed.");
      return Err(());
    }
  }

  let node_with_primitives = match find_filter_with_primitives(node) {
    Some(v) => v,
    None => return Err(()),
  };
  let primitives = collect_children(
    &node_with_primitives,
    primitive_units,
    state,
    object_bbox,
    rect,
    cache,
  );
  if primitives.is_empty() {
    return Err(());
  }

  let mut id = NonEmptyString::new(node.element_id().to_string()).ok_or(())?;
  // Generate ID only when we're parsing `objectBoundingBox` filter for the second time.
  if !cacheable && cache.filters.contains_key(id.get()) {
    id = cache.gen_id(IdKind::Filter);
  }
  let id_copy = id.get().to_string();

  let filter = Arc::new(Filter {
    id,
    rect,
    primitives,
  });

  cache.filters.insert(id_copy, filter.clone());

  Ok(Some(filter))
}

fn find_filter_with_primitives<'a>(node: SvgNode<'a>) -> Option<SvgNode<'a>> {
  for link in node.href_chain() {
    if link.tag() != Some(Tag::Filter) {
      log::warn!(
        "Filter '{}' cannot reference '{}' via 'xlink:href'.",
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

struct FilterResults {
  names: HashSet<String>,
  idx: usize,
}

fn collect_children(
  filter: &SvgNode,
  units: Units,
  state: &State,
  object_bbox: Option<NonZeroRect>,
  filter_region: NonZeroRect,
  cache: &mut Cache,
) -> Vec<Primitive> {
  let mut primitives = Vec::new();

  let mut results = FilterResults {
    names: HashSet::new(),
    idx: 1,
  };

  let scale = if units == Units::ObjectBoundingBox {
    if let Some(object_bbox) = object_bbox {
      object_bbox.size()
    } else {
      // No need to warn. Already checked.
      return Vec::new();
    }
  } else {
    match Size::from_wh(1.0, 1.0) {
      Some(size) => size,
      None => return Vec::new(),
    }
  };

  for child in filter.children() {
    let tag_name = match child.tag() {
      Some(v) => v,
      None => continue,
    };

    let filter_subregion =
      match resolve_primitive_region(child, tag_name, units, state, object_bbox, filter_region) {
        Some(v) => v,
        None => break,
      };

    let kind = match tag_name {
      Tag::FeDropShadow => convert_drop_shadow(child, scale, &primitives, state),
      Tag::FeGaussianBlur => convert_gaussian_blur(child, scale, &primitives),
      Tag::FeOffset => convert_offset(child, scale, &primitives),
      Tag::FeBlend => convert_blend(child, &primitives),
      Tag::FeFlood => convert_flood(child, state),
      Tag::FeComposite => convert_composite(child, &primitives),
      Tag::FeMerge => convert_merge(child, &primitives),
      Tag::FeTile => convert_tile(child, &primitives),
      Tag::FeImage => convert_image(child, filter_subregion, state, cache),
      Tag::FeComponentTransfer => convert_component_transfer(child, &primitives),
      Tag::FeColorMatrix => convert_color_matrix(child, &primitives),
      Tag::FeConvolveMatrix => {
        convert_convolve_matrix(child, &primitives).unwrap_or_else(create_dummy_primitive)
      }
      Tag::FeMorphology => convert_morphology(child, scale, &primitives),
      Tag::FeDisplacementMap => convert_displacement_map(child, scale, &primitives),
      Tag::FeTurbulence => convert_turbulence(child),
      Tag::FeDiffuseLighting => {
        convert_diffuse_lighting(child, &primitives, state).unwrap_or_else(create_dummy_primitive)
      }
      Tag::FeSpecularLighting => {
        convert_specular_lighting(child, &primitives, state).unwrap_or_else(create_dummy_primitive)
      }
      tag_name => {
        log::warn!(
          "'{}' is not a valid filter primitive. Skipped.",
          tag_name.name()
        );
        continue;
      }
    };

    let color_interpolation = match child.style().map(|style| style.color_interpolation_filters) {
      Some(StyleColorInterpolation::Srgb) => filter::ColorInterpolation::SRGB,
      _ => filter::ColorInterpolation::LinearRGB,
    };

    primitives.push(Primitive {
      rect: filter_subregion,
      color_interpolation,
      result: gen_result(child, &mut results),
      kind,
    });
  }

  // TODO: remove primitives which results are not used

  primitives
}

// TODO: rewrite/simplify/explain/whatever
fn resolve_primitive_region(
  fe: SvgNode,
  kind: Tag,
  units: Units,
  state: &State,
  bbox: Option<NonZeroRect>,
  filter_region: NonZeroRect,
) -> Option<NonZeroRect> {
  let x = fe.try_convert_length(Attr::X, units, state);
  let y = fe.try_convert_length(Attr::Y, units, state);
  let width = fe.try_convert_length(Attr::Width, units, state);
  let height = fe.try_convert_length(Attr::Height, units, state);

  let region = match kind {
    Tag::FeFlood | Tag::FeImage => {
      // `feImage` uses the object bbox.
      if units == Units::ObjectBoundingBox {
        let bbox = bbox?;

        // TODO: wrong
        // let ts_bbox = tiny_skia::Rect::new(ts.e, ts.f, ts.a, ts.d).unwrap();

        let r = NonZeroRect::from_xywh(
          x.unwrap_or(0.0),
          y.unwrap_or(0.0),
          width.unwrap_or(1.0),
          height.unwrap_or(1.0),
        )?;

        return Some(r.bbox_transform(bbox));
      } else {
        filter_region
      }
    }
    _ => filter_region,
  };

  // TODO: Wrong! Does not account rotate and skew.
  if units == Units::ObjectBoundingBox {
    let subregion_bbox = NonZeroRect::from_xywh(
      x.unwrap_or(0.0),
      y.unwrap_or(0.0),
      width.unwrap_or(1.0),
      height.unwrap_or(1.0),
    )?;

    Some(region.bbox_transform(subregion_bbox))
  } else {
    NonZeroRect::from_xywh(
      x.unwrap_or(region.x()),
      y.unwrap_or(region.y()),
      width.unwrap_or(region.width()),
      height.unwrap_or(region.height()),
    )
  }
}

// A malformed filter primitive usually should produce a transparent image.
// But since `FilterKind` structs are designed to always be valid,
// we are using `FeFlood` as fallback.
#[inline(never)]
pub(crate) fn create_dummy_primitive() -> Kind {
  Kind::Flood(Flood {
    color: Color::black(),
    opacity: Opacity::ZERO,
  })
}

#[inline(never)]
fn resolve_input(node: SvgNode, aid: Attr, primitives: &[Primitive]) -> Input {
  match node.parse(aid) {
    Some(s) => {
      let input = parse_in(s);

      // If `in` references an unknown `result` than fallback
      // to previous result or `SourceGraphic`.
      if let Input::Reference(ref name) = input
        && !primitives.iter().any(|p| p.result == *name)
      {
        return if let Some(prev) = primitives.last() {
          Input::Reference(prev.result.clone())
        } else {
          Input::SourceGraphic
        };
      }

      input
    }
    None => {
      if let Some(prev) = primitives.last() {
        // If `in` is not set and this is not the first primitive
        // than the input is a result of the previous primitive.
        Input::Reference(prev.result.clone())
      } else {
        // If `in` is not set and this is the first primitive
        // than the input is `SourceGraphic`.
        Input::SourceGraphic
      }
    }
  }
}

fn parse_in(s: &str) -> Input {
  match s {
    "SourceGraphic" => Input::SourceGraphic,
    "SourceAlpha" => Input::SourceAlpha,
    "BackgroundImage" | "BackgroundAlpha" | "FillPaint" | "StrokePaint" => {
      log::warn!("{} filter input isn't supported and not planed.", s);
      Input::SourceGraphic
    }
    _ => Input::Reference(s.to_string()),
  }
}

fn gen_result(node: SvgNode, results: &mut FilterResults) -> String {
  match node.parse::<&str>(Attr::Result) {
    Some(s) => {
      // Remember predefined result.
      results.names.insert(s.to_string());
      results.idx += 1;

      s.to_string()
    }
    None => {
      // Generate an unique name for `result`.
      loop {
        let name = format!("result{}", results.idx);
        results.idx += 1;

        if !results.names.contains(&name) {
          return name;
        }
      }
    }
  }
}

fn convert_blend(fe: SvgNode, primitives: &[Primitive]) -> Kind {
  let mode = fe.parse(Attr::Mode).unwrap_or_default();
  let input1 = resolve_input(fe, Attr::In, primitives);
  let input2 = resolve_input(fe, Attr::In2, primitives);
  Kind::Blend(Blend {
    mode,
    input1,
    input2,
  })
}

fn convert_color_matrix(fe: SvgNode, primitives: &[Primitive]) -> Kind {
  let kind = convert_color_matrix_kind(fe).unwrap_or_default();
  Kind::ColorMatrix(ColorMatrix {
    input: resolve_input(fe, Attr::In, primitives),
    kind,
  })
}

fn convert_color_matrix_kind(fe: SvgNode) -> Option<ColorMatrixKind> {
  match fe.parse(Attr::Type) {
    Some("saturate") => {
      if let Some(list) = fe.parse::<Vec<f32>>(Attr::Values) {
        if !list.is_empty() {
          let n = crate::svg::tree::f32_bound(0.0, list[0], 1.0);
          return Some(ColorMatrixKind::Saturate(positive(n)));
        } else {
          return Some(ColorMatrixKind::Saturate(positive(1.0)));
        }
      }
    }
    Some("hueRotate") => {
      if let Some(list) = fe.parse::<Vec<f32>>(Attr::Values) {
        if !list.is_empty() {
          return Some(ColorMatrixKind::HueRotate(list[0]));
        } else {
          return Some(ColorMatrixKind::HueRotate(0.0));
        }
      }
    }
    Some("luminanceToAlpha") => {
      return Some(ColorMatrixKind::LuminanceToAlpha);
    }
    _ => {
      // Fallback to `matrix`.
      if let Some(list) = fe.parse::<Vec<f32>>(Attr::Values)
        && list.len() == 20
      {
        return Some(ColorMatrixKind::Matrix(list));
      }
    }
  }

  None
}

fn convert_component_transfer(fe: SvgNode, primitives: &[Primitive]) -> Kind {
  let mut kind = ComponentTransfer {
    input: resolve_input(fe, Attr::In, primitives),
    func_r: TransferFunction::Identity,
    func_g: TransferFunction::Identity,
    func_b: TransferFunction::Identity,
    func_a: TransferFunction::Identity,
  };

  for child in fe.children().filter(SvgNode::is_element) {
    if let Some(func) = convert_transfer_function(child) {
      match child.tag() {
        Some(Tag::FeFuncR) => kind.func_r = func,
        Some(Tag::FeFuncG) => kind.func_g = func,
        Some(Tag::FeFuncB) => kind.func_b = func,
        Some(Tag::FeFuncA) => kind.func_a = func,
        _ => {}
      }
    }
  }

  Kind::ComponentTransfer(kind)
}

fn convert_transfer_function(node: SvgNode) -> Option<TransferFunction> {
  match node.parse(Attr::Type)? {
    "identity" => Some(TransferFunction::Identity),
    "table" => match node.parse::<Vec<f32>>(Attr::TableValues) {
      Some(values) => Some(TransferFunction::Table(values)),
      None => Some(TransferFunction::Table(Vec::new())),
    },
    "discrete" => match node.parse::<Vec<f32>>(Attr::TableValues) {
      Some(values) => Some(TransferFunction::Discrete(values)),
      None => Some(TransferFunction::Discrete(Vec::new())),
    },
    "linear" => Some(TransferFunction::Linear {
      slope: node.parse(Attr::Slope).unwrap_or(1.0),
      intercept: node.parse(Attr::Intercept).unwrap_or(0.0),
    }),
    "gamma" => Some(TransferFunction::Gamma {
      amplitude: node.parse(Attr::Amplitude).unwrap_or(1.0),
      exponent: node.parse(Attr::Exponent).unwrap_or(1.0),
      offset: node.parse(Attr::Offset).unwrap_or(0.0),
    }),
    _ => None,
  }
}

fn convert_composite(fe: SvgNode, primitives: &[Primitive]) -> Kind {
  let operator = match fe.parse(Attr::Operator).unwrap_or("over") {
    "in" => CompositeOperator::In,
    "out" => CompositeOperator::Out,
    "atop" => CompositeOperator::Atop,
    "xor" => CompositeOperator::Xor,
    "arithmetic" => CompositeOperator::Arithmetic {
      k1: fe.parse(Attr::K1).unwrap_or(0.0),
      k2: fe.parse(Attr::K2).unwrap_or(0.0),
      k3: fe.parse(Attr::K3).unwrap_or(0.0),
      k4: fe.parse(Attr::K4).unwrap_or(0.0),
    },
    _ => CompositeOperator::Over,
  };

  let input1 = resolve_input(fe, Attr::In, primitives);
  let input2 = resolve_input(fe, Attr::In2, primitives);

  Kind::Composite(Composite {
    operator,
    input1,
    input2,
  })
}

fn convert_convolve_matrix(fe: SvgNode, primitives: &[Primitive]) -> Option<Kind> {
  fn parse_target(target: Option<f32>, order: u32) -> Option<u32> {
    let default_target = (order as f32 / 2.0).floor() as u32;
    let target = target.unwrap_or(default_target as f32) as i32;
    if target < 0 || target >= order as i32 {
      None
    } else {
      Some(target as u32)
    }
  }

  let mut order_x = 3;
  let mut order_y = 3;
  if let Some(value) = fe.parse::<&str>(Attr::Order) {
    let mut scanner = Scanner::new(value);
    let x = scanner.number_f64().map_or(3, |n| n as i32);
    let y = scanner.number_f64().map_or(x, |n| n as i32);
    if x > 0 && y > 0 {
      order_x = x as u32;
      order_y = y as u32;
    }
  }

  let mut matrix = Vec::new();
  if let Some(list) = fe.parse::<Vec<f32>>(Attr::KernelMatrix)
    && order_x.checked_mul(order_y).map(|len| len as usize) == Some(list.len())
  {
    matrix = list;
  }

  let mut kernel_sum: f32 = matrix.iter().sum();
  // Round up to prevent float precision issues.
  kernel_sum = (kernel_sum * 1_000_000.0).round() / 1_000_000.0;
  if kernel_sum.approx_zero_ulps(4) {
    kernel_sum = 1.0;
  }

  let divisor = fe.parse(Attr::Divisor).unwrap_or(kernel_sum);
  if divisor.approx_zero_ulps(4) {
    return None;
  }

  let bias = fe.parse(Attr::Bias).unwrap_or(0.0);

  let target_x = parse_target(fe.parse(Attr::TargetX), order_x)?;
  let target_y = parse_target(fe.parse(Attr::TargetY), order_y)?;

  let kernel_matrix = ConvolveMatrixData::new(target_x, target_y, order_x, order_y, matrix)?;

  let edge_mode = match fe.parse(Attr::EdgeMode).unwrap_or("duplicate") {
    "none" => EdgeMode::None,
    "wrap" => EdgeMode::Wrap,
    _ => EdgeMode::Duplicate,
  };

  let preserve_alpha = fe.parse(Attr::PreserveAlpha).unwrap_or("false") == "true";

  Some(Kind::ConvolveMatrix(ConvolveMatrix {
    input: resolve_input(fe, Attr::In, primitives),
    matrix: kernel_matrix,
    divisor: NonZeroF32::new(divisor)?,
    bias,
    edge_mode,
    preserve_alpha,
  }))
}

fn convert_displacement_map(fe: SvgNode, scale: Size, primitives: &[Primitive]) -> Kind {
  let parse_channel = |aid| match fe.parse(aid).unwrap_or("A") {
    "R" => ColorChannel::R,
    "G" => ColorChannel::G,
    "B" => ColorChannel::B,
    _ => ColorChannel::A,
  };

  // TODO: should probably split scale to scale_x and scale_y,
  //       but resvg doesn't support displacement map anyway...
  let scale = (scale.width() + scale.height()) / 2.0;

  Kind::DisplacementMap(DisplacementMap {
    input1: resolve_input(fe, Attr::In, primitives),
    input2: resolve_input(fe, Attr::In2, primitives),
    scale: fe.parse(Attr::Scale).unwrap_or(0.0) * scale,
    x_channel_selector: parse_channel(Attr::XChannelSelector),
    y_channel_selector: parse_channel(Attr::YChannelSelector),
  })
}

/// The flood color of `fe` and its opacity, alpha of the color included.
fn flood(fe: SvgNode, state: &State) -> (Color, Opacity) {
  let Some(style) = fe.style() else {
    return (Color::black(), Opacity::ONE);
  };
  let (color, opacity) = split_alpha(match style.flood_color {
    ColorInput::Value(color) => color,
    ColorInput::CurrentColor => resolve_current_color(fe, state),
  });

  (color, opacity * Opacity::new_clamped(style.flood_opacity))
}

fn convert_drop_shadow(fe: SvgNode, scale: Size, primitives: &[Primitive], state: &State) -> Kind {
  let (std_dev_x, std_dev_y) = convert_std_dev_attr(fe, scale, "2 2");
  let (color, opacity) = flood(fe, state);

  Kind::DropShadow(DropShadow {
    input: resolve_input(fe, Attr::In, primitives),
    dx: fe.parse(Attr::Dx).unwrap_or(2.0) * scale.width(),
    dy: fe.parse(Attr::Dy).unwrap_or(2.0) * scale.height(),
    std_dev_x,
    std_dev_y,
    color,
    opacity,
  })
}

fn convert_flood(fe: SvgNode, state: &State) -> Kind {
  let (color, opacity) = flood(fe, state);

  Kind::Flood(Flood { color, opacity })
}

fn convert_gaussian_blur(fe: SvgNode, scale: Size, primitives: &[Primitive]) -> Kind {
  let (std_dev_x, std_dev_y) = convert_std_dev_attr(fe, scale, "0 0");
  Kind::GaussianBlur(GaussianBlur {
    input: resolve_input(fe, Attr::In, primitives),
    std_dev_x,
    std_dev_y,
  })
}

fn convert_std_dev_attr(fe: SvgNode, scale: Size, default: &str) -> (PositiveF32, PositiveF32) {
  let text = fe.parse(Attr::StdDeviation).unwrap_or(default);
  let mut scanner = Scanner::new(text);

  let n1 = scanner.number_f64();
  let n2 = scanner.number_f64();
  // `stdDeviation` must have no more than two values.
  // Otherwise we should fallback to `0 0`.
  let n3 = scanner.number_f64();

  let (std_dev_x, std_dev_y) = match (n1, n2, n3) {
    (Some(n1), Some(n2), None) => (n1, n2),
    (Some(n1), None, None) => (n1, n1),
    _ => (0.0, 0.0),
  };

  let std_dev_x = (std_dev_x as f32) * scale.width();
  let std_dev_y = (std_dev_y as f32) * scale.height();

  let std_dev_x = PositiveF32::new(std_dev_x).unwrap_or(PositiveF32::ZERO);
  let std_dev_y = PositiveF32::new(std_dev_y).unwrap_or(PositiveF32::ZERO);

  (std_dev_x, std_dev_y)
}

fn convert_image(
  fe: SvgNode,
  filter_subregion: NonZeroRect,
  state: &State,
  cache: &mut Cache,
) -> Kind {
  match convert_image_inner(fe, filter_subregion, state, cache) {
    Some(kind) => kind,
    None => create_dummy_primitive(),
  }
}

fn convert_image_inner(
  fe: SvgNode,
  filter_subregion: NonZeroRect,
  state: &State,
  cache: &mut Cache,
) -> Option<Kind> {
  let rendering_mode = fe.style()?.image_rendering;

  if let Some(node) = fe.try_parse::<SvgNode>(Attr::Href) {
    let mut state = state.clone();
    state.fe_image_link = true;
    let mut root = Group::empty();
    convert_element(node, &state, cache, &mut root);
    return if root.has_children() {
      root.calculate_bounding_boxes();
      // Transfer node id from group's child to the group itself if needed.
      if let Some(Node::Group(g)) = root.children.first_mut()
        && let Some(child2) = g.children.first_mut()
      {
        g.id = child2.id().to_string();
        match child2 {
          Node::Group(g2) => g2.id.clear(),
          Node::Path(path) => path.id.clear(),
          Node::Image(image) => image.id.clear(),
          Node::Text(text) => text.id.clear(),
        }
      }

      Some(Kind::Image(Image { root }))
    } else {
      None
    };
  }

  let href = fe.try_parse(Attr::Href).log_none(|| {
    log::warn!("The 'feImage' element lacks the 'xlink:href' attribute. Skipped.");
  })?;
  let img_data = image::get_href_data(href, state)?;
  let actual_size = img_data.actual_size()?;

  let aspect: PreserveAspectRatio = fe.parse(Attr::PreserveAspectRatio).unwrap_or_default();

  let mut root = Group::empty();
  image::convert_inner(
    img_data,
    ImagePlacement {
      id: cache.gen_id(IdKind::Image).take(),
      visible: true,
      rendering_mode,
      aspect,
      actual_size,
      rect: filter_subregion.translate_to(0.0, 0.0)?,
    },
    cache,
    &mut root,
  );
  root.calculate_bounding_boxes();

  Some(Kind::Image(Image { root }))
}

fn convert_diffuse_lighting(fe: SvgNode, primitives: &[Primitive], state: &State) -> Option<Kind> {
  let light_source = convert_light_source(fe)?;
  Some(Kind::DiffuseLighting(DiffuseLighting {
    input: resolve_input(fe, Attr::In, primitives),
    surface_scale: fe.parse(Attr::SurfaceScale).unwrap_or(1.0),
    diffuse_constant: fe.parse(Attr::DiffuseConstant).unwrap_or(1.0),
    lighting_color: convert_lighting_color(fe, state),
    light_source,
  }))
}

fn convert_specular_lighting(fe: SvgNode, primitives: &[Primitive], state: &State) -> Option<Kind> {
  let light_source = convert_light_source(fe)?;

  let specular_exponent = fe.parse(Attr::SpecularExponent).unwrap_or(1.0);
  if !(1.0..=128.0).contains(&specular_exponent) {
    // When exponent is out of range, the whole filter primitive should be ignored.
    return None;
  }

  let specular_exponent = crate::svg::tree::f32_bound(1.0, specular_exponent, 128.0);

  Some(Kind::SpecularLighting(SpecularLighting {
    input: resolve_input(fe, Attr::In, primitives),
    surface_scale: fe.parse(Attr::SurfaceScale).unwrap_or(1.0),
    specular_constant: fe.parse(Attr::SpecularConstant).unwrap_or(1.0),
    specular_exponent,
    lighting_color: convert_lighting_color(fe, state),
    light_source,
  }))
}

#[inline(never)]
fn convert_lighting_color(node: SvgNode, state: &State) -> Color {
  // Color's alpha doesn't affect lighting-color. Simply skip it.
  match node.style().map(|style| style.lighting_color) {
    Some(ColorInput::Value(color)) => split_alpha(color).0,
    Some(ColorInput::CurrentColor) => split_alpha(resolve_current_color(node, state)).0,
    None => Color::white(),
  }
}

#[inline(never)]
fn convert_light_source(parent: SvgNode) -> Option<LightSource> {
  let child = parent.children().find(|n| {
    matches!(
      n.tag(),
      Some(Tag::FeDistantLight) | Some(Tag::FePointLight) | Some(Tag::FeSpotLight)
    )
  })?;

  match child.tag() {
    Some(Tag::FeDistantLight) => Some(LightSource::DistantLight(DistantLight {
      azimuth: child.parse(Attr::Azimuth).unwrap_or(0.0),
      elevation: child.parse(Attr::Elevation).unwrap_or(0.0),
    })),
    Some(Tag::FePointLight) => Some(LightSource::PointLight(PointLight {
      x: child.parse(Attr::X).unwrap_or(0.0),
      y: child.parse(Attr::Y).unwrap_or(0.0),
      z: child.parse(Attr::Z).unwrap_or(0.0),
    })),
    Some(Tag::FeSpotLight) => {
      let specular_exponent = child.parse(Attr::SpecularExponent).unwrap_or(1.0);
      let specular_exponent = PositiveF32::new(specular_exponent).unwrap_or_else(|| positive(1.0));

      Some(LightSource::SpotLight(SpotLight {
        x: child.parse(Attr::X).unwrap_or(0.0),
        y: child.parse(Attr::Y).unwrap_or(0.0),
        z: child.parse(Attr::Z).unwrap_or(0.0),
        points_at_x: child.parse(Attr::PointsAtX).unwrap_or(0.0),
        points_at_y: child.parse(Attr::PointsAtY).unwrap_or(0.0),
        points_at_z: child.parse(Attr::PointsAtZ).unwrap_or(0.0),
        specular_exponent,
        limiting_cone_angle: child.parse(Attr::LimitingConeAngle),
      }))
    }
    _ => None,
  }
}

fn convert_merge(fe: SvgNode, primitives: &[Primitive]) -> Kind {
  let mut inputs = Vec::new();
  for child in fe.children() {
    inputs.push(resolve_input(child, Attr::In, primitives));
  }

  Kind::Merge(Merge { inputs })
}

fn convert_morphology(fe: SvgNode, scale: Size, primitives: &[Primitive]) -> Kind {
  let operator = match fe.parse(Attr::Operator).unwrap_or("erode") {
    "dilate" => MorphologyOperator::Dilate,
    _ => MorphologyOperator::Erode,
  };

  let mut radius_x = positive(scale.width());
  let mut radius_y = positive(scale.height());
  if let Some(list) = fe.parse::<Vec<f32>>(Attr::Radius) {
    let mut rx = 0.0;
    let mut ry = 0.0;
    if list.len() == 2 {
      rx = list[0];
      ry = list[1];
    } else if list.len() == 1 {
      rx = list[0];
      ry = list[0]; // The same as `rx`.
    }

    if rx.approx_zero_ulps(4) && ry.approx_zero_ulps(4) {
      rx = 1.0;
      ry = 1.0;
    }

    // If only one of the values is zero, reset it to 1.0
    // This is not specified in the spec, but this is how Chrome and Safari work.
    if rx.approx_zero_ulps(4) && !ry.approx_zero_ulps(4) {
      rx = 1.0;
    }
    if !rx.approx_zero_ulps(4) && ry.approx_zero_ulps(4) {
      ry = 1.0;
    }

    // Both values must be positive.
    if rx.is_sign_positive() && ry.is_sign_positive() {
      radius_x = positive(rx * scale.width());
      radius_y = positive(ry * scale.height());
    }
  }

  Kind::Morphology(Morphology {
    input: resolve_input(fe, Attr::In, primitives),
    operator,
    radius_x,
    radius_y,
  })
}

fn convert_offset(fe: SvgNode, scale: Size, primitives: &[Primitive]) -> Kind {
  Kind::Offset(Offset {
    input: resolve_input(fe, Attr::In, primitives),
    dx: fe.parse(Attr::Dx).unwrap_or(0.0) * scale.width(),
    dy: fe.parse(Attr::Dy).unwrap_or(0.0) * scale.height(),
  })
}

fn convert_tile(fe: SvgNode, primitives: &[Primitive]) -> Kind {
  Kind::Tile(Tile {
    input: resolve_input(fe, Attr::In, primitives),
  })
}

fn convert_turbulence(fe: SvgNode) -> Kind {
  let mut base_frequency_x = PositiveF32::ZERO;
  let mut base_frequency_y = PositiveF32::ZERO;
  if let Some(list) = fe.parse::<Vec<f32>>(Attr::BaseFrequency) {
    let mut x = 0.0;
    let mut y = 0.0;
    if list.len() == 2 {
      x = list[0];
      y = list[1];
    } else if list.len() == 1 {
      x = list[0];
      y = list[0]; // The same as `x`.
    }

    if x.is_sign_positive() && y.is_sign_positive() {
      base_frequency_x = positive(x);
      base_frequency_y = positive(y);
    }
  }

  let mut num_octaves = fe.parse(Attr::NumOctaves).unwrap_or(1.0);
  if num_octaves.is_sign_negative() {
    num_octaves = 0.0;
  }

  let kind = match fe.parse(Attr::Type).unwrap_or("turbulence") {
    "fractalNoise" => TurbulenceKind::FractalNoise,
    _ => TurbulenceKind::Turbulence,
  };

  Kind::Turbulence(Turbulence {
    base_frequency_x,
    base_frequency_y,
    num_octaves: num_octaves.round() as u32,
    seed: fe.parse::<f32>(Attr::Seed).unwrap_or(0.0).trunc() as i32,
    stitch_tiles: fe.parse(Attr::StitchTiles) == Some("stitch"),
    kind,
  })
}

#[inline(never)]
fn convert_grayscale_function(amount: f32) -> Kind {
  let amount = amount.min(1.0);
  Kind::ColorMatrix(ColorMatrix {
    input: Input::SourceGraphic,
    kind: ColorMatrixKind::Matrix(vec![
      (0.2126 + 0.7874 * (1.0 - amount)),
      (0.7152 - 0.7152 * (1.0 - amount)),
      (0.0722 - 0.0722 * (1.0 - amount)),
      0.0,
      0.0,
      (0.2126 - 0.2126 * (1.0 - amount)),
      (0.7152 + 0.2848 * (1.0 - amount)),
      (0.0722 - 0.0722 * (1.0 - amount)),
      0.0,
      0.0,
      (0.2126 - 0.2126 * (1.0 - amount)),
      (0.7152 - 0.7152 * (1.0 - amount)),
      (0.0722 + 0.9278 * (1.0 - amount)),
      0.0,
      0.0,
      0.0,
      0.0,
      0.0,
      1.0,
      0.0,
    ]),
  })
}

#[inline(never)]
fn convert_sepia_function(amount: f32) -> Kind {
  let amount = amount.min(1.0);
  Kind::ColorMatrix(ColorMatrix {
    input: Input::SourceGraphic,
    kind: ColorMatrixKind::Matrix(vec![
      (0.393 + 0.607 * (1.0 - amount)),
      (0.769 - 0.769 * (1.0 - amount)),
      (0.189 - 0.189 * (1.0 - amount)),
      0.0,
      0.0,
      (0.349 - 0.349 * (1.0 - amount)),
      (0.686 + 0.314 * (1.0 - amount)),
      (0.168 - 0.168 * (1.0 - amount)),
      0.0,
      0.0,
      (0.272 - 0.272 * (1.0 - amount)),
      (0.534 - 0.534 * (1.0 - amount)),
      (0.131 + 0.869 * (1.0 - amount)),
      0.0,
      0.0,
      0.0,
      0.0,
      0.0,
      1.0,
      0.0,
    ]),
  })
}

#[inline(never)]
fn convert_saturate_function(amount: f32) -> Kind {
  let amount = PositiveF32::new(amount).unwrap_or(PositiveF32::ZERO);
  Kind::ColorMatrix(ColorMatrix {
    input: Input::SourceGraphic,
    kind: ColorMatrixKind::Saturate(amount),
  })
}

#[inline(never)]
fn convert_hue_rotate_function(degrees: f32) -> Kind {
  Kind::ColorMatrix(ColorMatrix {
    input: Input::SourceGraphic,
    kind: ColorMatrixKind::HueRotate(degrees),
  })
}

#[inline(never)]
fn convert_invert_function(amount: f32) -> Kind {
  let amount = amount.min(1.0);
  Kind::ComponentTransfer(ComponentTransfer {
    input: Input::SourceGraphic,
    func_r: TransferFunction::Table(vec![amount, 1.0 - amount]),
    func_g: TransferFunction::Table(vec![amount, 1.0 - amount]),
    func_b: TransferFunction::Table(vec![amount, 1.0 - amount]),
    func_a: TransferFunction::Identity,
  })
}

#[inline(never)]
fn convert_opacity_function(amount: f32) -> Kind {
  let amount = amount.min(1.0);
  Kind::ComponentTransfer(ComponentTransfer {
    input: Input::SourceGraphic,
    func_r: TransferFunction::Identity,
    func_g: TransferFunction::Identity,
    func_b: TransferFunction::Identity,
    func_a: TransferFunction::Table(vec![0.0, amount]),
  })
}

#[inline(never)]
fn convert_brightness_function(amount: f32) -> Kind {
  Kind::ComponentTransfer(ComponentTransfer {
    input: Input::SourceGraphic,
    func_r: TransferFunction::Linear {
      slope: amount,
      intercept: 0.0,
    },
    func_g: TransferFunction::Linear {
      slope: amount,
      intercept: 0.0,
    },
    func_b: TransferFunction::Linear {
      slope: amount,
      intercept: 0.0,
    },
    func_a: TransferFunction::Identity,
  })
}

#[inline(never)]
fn convert_contrast_function(amount: f32) -> Kind {
  Kind::ComponentTransfer(ComponentTransfer {
    input: Input::SourceGraphic,
    func_r: TransferFunction::Linear {
      slope: amount,
      intercept: -(0.5 * amount) + 0.5,
    },
    func_g: TransferFunction::Linear {
      slope: amount,
      intercept: -(0.5 * amount) + 0.5,
    },
    func_b: TransferFunction::Linear {
      slope: amount,
      intercept: -(0.5 * amount) + 0.5,
    },
    func_a: TransferFunction::Identity,
  })
}

#[inline(never)]
fn convert_blur_function(node: SvgNode, std_dev: StyleLength, state: &State) -> Kind {
  let std_dev = PositiveF32::new(css_length(node, std_dev, state)).unwrap_or(PositiveF32::ZERO);

  Kind::GaussianBlur(GaussianBlur {
    input: Input::SourceGraphic,
    std_dev_x: std_dev,
    std_dev_y: std_dev,
  })
}

#[inline(never)]
fn convert_drop_shadow_function(node: SvgNode, shadow: &TextShadow, state: &State) -> Kind {
  let std_dev =
    PositiveF32::new(css_length(node, shadow.blur_radius, state)).unwrap_or(PositiveF32::ZERO);
  let (color, opacity) = split_alpha(match shadow.color {
    ColorInput::Value(color) => color,
    ColorInput::CurrentColor => resolve_current_color(node, state),
  });

  Kind::DropShadow(DropShadow {
    input: Input::SourceGraphic,
    dx: css_length(node, shadow.offset_x, state),
    dy: css_length(node, shadow.offset_y, state),
    std_dev_x: std_dev,
    std_dev_y: std_dev,
    color,
    opacity,
  })
}

/// A CSS length of a filter function in user units, through the units SVG lengths know.
fn css_length(node: SvgNode, length: StyleLength, state: &State) -> f32 {
  let (number, unit) = match length {
    StyleLength::Px(value) => (value, LengthUnit::Px),
    StyleLength::Em(value) => (value, LengthUnit::Em),
    StyleLength::Cm(value) => (value, LengthUnit::Cm),
    StyleLength::Mm(value) => (value, LengthUnit::Mm),
    StyleLength::In(value) => (value, LengthUnit::In),
    StyleLength::Pt(value) => (value, LengthUnit::Pt),
    StyleLength::Pc(value) => (value, LengthUnit::Pc),
    StyleLength::Q(value) => (value / 4.0, LengthUnit::Mm),
    _ => return 0.0,
  };

  units::convert_other_length(Length::new(number, unit), node, state)
}

/// A non-negative number, zero when it is not one.
fn positive(value: f32) -> PositiveF32 {
  PositiveF32::new(value).unwrap_or(PositiveF32::ZERO)
}
