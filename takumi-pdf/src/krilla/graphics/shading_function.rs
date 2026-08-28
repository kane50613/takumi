use std::hash::{Hash, Hasher};
use std::ops::DerefMut;
use std::sync::Arc;

use bumpalo::Bump;
use pdf_writer::types::{FunctionShadingType, PostScriptOp};
use pdf_writer::{Chunk, Finish, Name, Ref};
use tiny_skia_path::Point;

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::configure::ValidationError;
use crate::krilla::geom::{Rect, Transform};
use crate::krilla::graphics::color::luma;
use crate::krilla::graphics::color::{Color, ColorSpace};
use crate::krilla::graphics::paint::{LinearGradient, RadialGradient, SweepGradient};
use crate::krilla::graphics::paint::{SpreadMethod, Stop};
use crate::krilla::num::NormalizedF32;
use crate::krilla::resource;
use crate::krilla::resource::Resourceable;
use crate::krilla::serialize::{Cacheable, MaybeDeviceColorSpace, SerializeContext};
use crate::krilla::stream::FilterStreamBuilder;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub(crate) enum GradientType {
  Sweep,
  Linear,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct RadialAxialGradient {
  pub(crate) coords: Vec<f32>,
  pub(crate) shading_type: FunctionShadingType,
  pub(crate) stops: Vec<Stop>,
  pub(crate) anti_alias: bool,
}

impl Eq for RadialAxialGradient {}

impl Hash for RadialAxialGradient {
  fn hash<H: Hasher>(&self, state: &mut H) {
    for el in &self.coords {
      el.to_bits().hash(state);
    }

    self.shading_type.hash(state);
    self.stops.hash(state);
    self.anti_alias.hash(state);
  }
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct PostScriptGradient {
  pub(crate) min: f32,
  pub(crate) max: f32,
  // Only used for sweep gradient
  pub(crate) cx: f32,
  // Only used for sweep gradient
  pub(crate) cy: f32,
  pub(crate) stops: Vec<Stop>,
  pub(crate) domain: Rect,
  pub(crate) spread_method: SpreadMethod,
  pub(crate) gradient_type: GradientType,
  pub(crate) anti_alias: bool,
}

impl Eq for PostScriptGradient {}

impl Hash for PostScriptGradient {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.min.to_bits().hash(state);
    self.max.to_bits().hash(state);
    self.stops.hash(state);
    self.domain.hash(state);
    self.spread_method.hash(state);
    self.gradient_type.hash(state);
    self.anti_alias.hash(state);
  }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub(crate) enum GradientProperties {
  RadialAxialGradient(RadialAxialGradient),
  PostScriptGradient(PostScriptGradient),
}

impl GradientProperties {
  // Check if the gradient could be encoded as a solid fill instead.
  pub(crate) fn single_stop_color(&self) -> Option<(&Color, NormalizedF32)> {
    match self {
      GradientProperties::RadialAxialGradient(rag) => {
        if rag.stops.len() == 1 {
          return Some((&rag.stops[0].color, rag.stops[0].opacity));
        }
      }
      GradientProperties::PostScriptGradient(psg) => {
        if psg.stops.len() == 1 {
          return Some((&psg.stops[0].color, psg.stops[0].opacity));
        }
      }
    }

    None
  }
}

pub(crate) trait GradientPropertiesExt {
  fn gradient_properties(self, bbox: Rect) -> (GradientProperties, Transform);
}

fn get_expanded_bbox(mut bbox: Rect, shading_transform: Transform) -> Rect {
  // We need to make sure the shading covers the whole bbox of the object after
  // the transform as been applied. In order to know that, we need to calculate the
  // resulting bbox from the inverted transform. A singular transform (e.g.
  // `gradientTransform="scale(0)"`) is not invertible; the shading collapses to
  // nothing, so the unexpanded bbox is enough.
  if let Some(expanded) = shading_transform
    .invert()
    .and_then(|inverted| bbox.transform(inverted))
  {
    bbox.expand(&expanded);
  }
  bbox
}

/// When writing a PostScript shading, we assume that both points are on a horizontal
/// line. Here, we calculate by how much we need to rotate the second point so that it
/// is horizontal to the first point, as well as the position of the rotated point.
fn get_point_ts(start: Point, end: Point) -> (Transform, f32, f32) {
  let dist = start.distance(end);

  let dx = end.x - start.x;
  let dy = end.y - start.y;
  let angle = dy.atan2(dx).to_degrees();

  (
    Transform::from_rotate_at(angle, start.x, start.y),
    start.x,
    start.x + dist,
  )
}

impl GradientPropertiesExt for LinearGradient {
  fn gradient_properties(self, bbox: Rect) -> (GradientProperties, Transform) {
    if self.spread_method == SpreadMethod::Pad {
      (
        GradientProperties::RadialAxialGradient(RadialAxialGradient {
          coords: vec![self.x1, self.y1, self.x2, self.y2],
          shading_type: FunctionShadingType::Axial,
          stops: self.stops,
          anti_alias: self.anti_alias,
        }),
        self.transform,
      )
    } else {
      let p1 = Point::from_xy(self.x1, self.y1);
      let p2 = Point::from_xy(self.x2, self.y2);

      let (ts, min, max) = get_point_ts(p1, p2);
      (
        GradientProperties::PostScriptGradient(PostScriptGradient {
          min,
          max,
          cx: 0.0,
          cy: 0.0,
          stops: self.stops,
          domain: get_expanded_bbox(bbox, self.transform.pre_concat(ts)),
          spread_method: self.spread_method,
          gradient_type: GradientType::Linear,
          anti_alias: self.anti_alias,
        }),
        self.transform.pre_concat(ts),
      )
    }
  }
}

impl GradientPropertiesExt for SweepGradient {
  fn gradient_properties(self, bbox: Rect) -> (GradientProperties, Transform) {
    let min = self.start_angle;
    let max = self.end_angle;

    let transform = self.transform;

    (
      GradientProperties::PostScriptGradient(PostScriptGradient {
        min,
        max,
        cx: self.cx,
        cy: self.cy,
        stops: self.stops,
        domain: get_expanded_bbox(bbox, transform),
        spread_method: self.spread_method,
        gradient_type: GradientType::Sweep,
        anti_alias: self.anti_alias,
      }),
      transform,
    )
  }
}

impl GradientPropertiesExt for RadialGradient {
  fn gradient_properties(self, _: Rect) -> (GradientProperties, Transform) {
    // TODO: Support other spread methods
    (
      GradientProperties::RadialAxialGradient(RadialAxialGradient {
        coords: vec![self.fx, self.fy, self.fr, self.cx, self.cy, self.cr],
        shading_type: FunctionShadingType::Radial,
        stops: self.stops,
        anti_alias: self.anti_alias,
      }),
      self.transform,
    )
  }
}

#[derive(Debug, Hash, Eq, PartialEq)]
struct Repr {
  pub(crate) properties: GradientProperties,
  pub(crate) use_opacities: bool,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub(crate) struct ShadingFunction(Arc<Repr>);

impl ShadingFunction {
  pub(crate) fn new(properties: GradientProperties, use_opacities: bool) -> Self {
    Self(Arc::new(Repr {
      properties,
      use_opacities,
    }))
  }
}

impl Cacheable for ShadingFunction {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    let mut stream_chunk = sc.new_chunk();

    match &self.0.properties {
      GradientProperties::RadialAxialGradient(rag) => {
        let shading_cs = shading_color_space(sc, rag.stops[0].color.clone(), self.0.use_opacities);
        let registered_cs = sc.register_colorspace(chunk_container, shading_cs);
        let chunk = &mut chunk_container.non_stream.shading_functions;
        serialize_axial_radial_shading(
          sc,
          chunk,
          root_ref,
          rag,
          self.0.use_opacities,
          registered_cs,
        )
      }
      GradientProperties::PostScriptGradient(psg) => {
        sc.register_validation_error(ValidationError::ContainsPostScript(sc.location));
        let shading_cs = shading_color_space(sc, psg.stops[0].color.clone(), self.0.use_opacities);
        let registered_cs = sc.register_colorspace(chunk_container, shading_cs);
        let chunk = &mut chunk_container.non_stream.shading_functions;
        serialize_postscript_shading(
          sc,
          chunk,
          &mut stream_chunk,
          root_ref,
          psg,
          self.0.use_opacities,
          registered_cs,
        )
      }
    }

    // Note: The stream chunk might be empty.
    chunk_container.streams.shading_functions.push(stream_chunk);
  }
}

impl Resourceable for ShadingFunction {
  type Resource = resource::Shading;
}

fn shading_color_space(sc: &mut SerializeContext, color: Color, use_opacities: bool) -> ColorSpace {
  if use_opacities {
    luma::color_space(sc.serialize_settings().no_device_cs).into()
  } else {
    color.color_space(sc)
  }
}

fn serialize_postscript_shading(
  sc: &mut SerializeContext,
  chunk: &mut Chunk,
  stream_chunk: &mut Chunk,
  root_ref: Ref,
  post_script_gradient: &PostScriptGradient,
  use_opacities: bool,
  cs: MaybeDeviceColorSpace,
) {
  let domain = post_script_gradient.domain;

  let bump = Bump::new();
  let function_ref =
    select_postscript_function(post_script_gradient, stream_chunk, sc, &bump, use_opacities);
  let mut shading = chunk.function_shading(root_ref);
  shading.shading_type(FunctionShadingType::Function);

  cs.write(shading.deref_mut().insert(Name(b"ColorSpace")));

  // Write the identity matrix, because ghostscript has a bug where
  // it thinks the entry is mandatory.
  shading.matrix([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
  shading.anti_alias(post_script_gradient.anti_alias);
  shading.function(function_ref);

  shading.domain([domain.left(), domain.right(), domain.top(), domain.bottom()]);
  shading.finish();
}

fn serialize_axial_radial_shading(
  sc: &mut SerializeContext,
  chunk: &mut Chunk,
  root_ref: Ref,
  radial_axial_gradient: &RadialAxialGradient,
  use_opacities: bool,
  cs: MaybeDeviceColorSpace,
) {
  let function_ref = select_axial_radial_function(radial_axial_gradient, chunk, sc, use_opacities);
  let mut shading = chunk.function_shading(root_ref);
  if radial_axial_gradient.shading_type == FunctionShadingType::Radial {
    shading.shading_type(FunctionShadingType::Radial);
  } else {
    shading.shading_type(FunctionShadingType::Axial);
  }

  cs.write(shading.deref_mut().insert(Name(b"ColorSpace")));

  shading.anti_alias(radial_axial_gradient.anti_alias);
  shading.function(function_ref);
  shading.coords(radial_axial_gradient.coords.iter().copied());
  shading.extend([true, true]);
  shading.finish();
}

fn select_axial_radial_function(
  properties: &RadialAxialGradient,
  chunk: &mut Chunk,
  sc: &mut SerializeContext,
  use_opacities: bool,
) -> Ref {
  debug_assert!(properties.stops.len() > 1);

  let mut stops = properties.stops.clone();

  if let Some(first) = stops.first() {
    if first.offset.get() != 0.0 {
      let mut new_stop = first.clone();
      new_stop.offset = NormalizedF32::ZERO;
      stops.insert(0, new_stop);
    }
  }

  if let Some(last) = stops.last() {
    if last.offset.get() != 1.0 {
      let mut new_stop = last.clone();
      new_stop.offset = NormalizedF32::ONE;
      stops.push(new_stop);
    }
  }

  if stops.len() == 2 {
    if use_opacities {
      serialize_exponential(
        vec![stops[0].opacity.get()],
        vec![stops[1].opacity.get()],
        chunk,
        sc,
      )
    } else {
      serialize_exponential(
        stops[0]
          .color
          .to_pdf_color()
          .into_iter()
          .collect::<Vec<_>>(),
        stops[1]
          .color
          .to_pdf_color()
          .into_iter()
          .collect::<Vec<_>>(),
        chunk,
        sc,
      )
    }
  } else {
    serialize_stitching(&stops, chunk, sc, use_opacities)
  }
}

fn select_postscript_function(
  properties: &PostScriptGradient,
  chunk: &mut Chunk,
  sc: &mut SerializeContext,
  bump: &Bump,
  use_opacities: bool,
) -> Ref {
  debug_assert!(properties.stops.len() > 1);

  if properties.gradient_type == GradientType::Linear {
    serialize_linear_postscript(properties, chunk, sc, use_opacities)
  } else if properties.gradient_type == GradientType::Sweep {
    serialize_sweep_postscript(properties, chunk, sc, bump, use_opacities)
  } else {
    todo!();
  }
}

// Not working yet
// fn serialize_radial_postscript(
//     properties: &GradientProperties,
//     sc: &mut SerializeContext,
//     bbox: &Rect,
// ) -> Ref {
// let root_ref = sc.new_ref();
//
// let start_code = [
//     "{".to_string(),
//     // Stack: x y
//     "80 exch 80 sub dup mul 3 1 roll sub dup mul add sqrt 120 div 0 0".to_string(),
// ];
//
// let end_code = ["}".to_string()];
//
// let mut code = Vec::new();
// code.extend(start_code);
// // code.push(encode_spread_method(min, max, properties.spread_method));
// // code.push(encode_stops(&properties.stops, min, max));
// code.extend(end_code);
//
// let code = code.join(" ").into_bytes();
// let mut postscript_function = sc.chunk_mut().post_script_function(root_ref, &code);
// postscript_function.domain([bbox.left(), bbox.right(), bbox.top(), bbox.bottom()]);
// postscript_function.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
//
// root_ref
// }

fn serialize_sweep_postscript(
  properties: &PostScriptGradient,
  chunk: &mut Chunk,
  sc: &mut SerializeContext,
  bump: &Bump,
  use_opacities: bool,
) -> Ref {
  use pdf_writer::types::PostScriptOp::*;

  let root_ref = sc.new_ref();

  let min: f32 = properties.min;
  let max: f32 = properties.max;

  let mut code = vec![];
  code.extend([
    // Stack: x y
    // Shift by cy, so gradient actually starts from the center
    Real(properties.cy),
    Sub,
    Exch,
    // y x
    // Shift by cx, so gradient actually starts from the center
    Real(properties.cx),
    Sub,
    // Make sure x is never 0.
    Dup,
    Dup,
    Real(0.0001),
    Lt,
    Exch,
    Real(-0.0001),
    Gt,
    And,
    If(bump.alloc([Pop, Real(0.0001)])),
    // Get the angle
    Atan,
  ]);

  encode_spread_method(min, max, &mut code, bump, properties.spread_method);
  encode_postscript_stops(&properties.stops, min, max, &mut code, bump, use_opacities);

  let encoded = PostScriptOp::encode(&code);
  sc.register_limits(encoded.limits());
  let encoded = FilterStreamBuilder::new_from_content_stream(&encoded, &sc.serialize_settings())
    .finish(&sc.serialize_settings());
  let mut postscript_function = chunk.post_script_function(root_ref, encoded.encoded_data());
  encoded.write_filters(postscript_function.deref_mut().deref_mut());
  postscript_function.domain([
    properties.domain.left(),
    properties.domain.right(),
    properties.domain.top(),
    properties.domain.bottom(),
  ]);

  if use_opacities {
    postscript_function.range([0.0, 1.0]);
  } else {
    postscript_function.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
  }

  root_ref
}

const MAX_POSTSCRIPT_STOPS: usize = 97;

fn trim_stops(stops: &[Stop]) -> Vec<Stop> {
  let len = stops.len();
  let factor = len as f32 / MAX_POSTSCRIPT_STOPS as f32;
  let mut cur_index: f32 = 0.0;

  let get_index = |i: f32| i.round() as usize;

  let mut new_stops = vec![];

  while get_index(cur_index) < stops.len() {
    new_stops.push(stops[get_index(cur_index)].clone());
    cur_index += factor;
  }

  new_stops
}

fn serialize_linear_postscript(
  properties: &PostScriptGradient,
  chunk: &mut Chunk,
  sc: &mut SerializeContext,
  use_opacities: bool,
) -> Ref {
  use pdf_writer::types::PostScriptOp::*;

  let bump = Bump::new();
  let root_ref = sc.new_ref();

  let min: f32 = properties.min;
  let max: f32 = properties.max;

  let mut code = vec![];
  code.extend([
    // Stack: x y
    // Ignore the y coordinate. We account for it in the gradient transform.
    Pop,
    // x
  ]);

  encode_spread_method(min, max, &mut code, &bump, properties.spread_method);
  encode_postscript_stops(&properties.stops, min, max, &mut code, &bump, use_opacities);

  let encoded = PostScriptOp::encode(&code);
  sc.register_limits(encoded.limits());
  let encoded = FilterStreamBuilder::new_from_content_stream(&encoded, &sc.serialize_settings())
    .finish(&sc.serialize_settings());
  let mut postscript_function = chunk.post_script_function(root_ref, encoded.encoded_data());
  encoded.write_filters(postscript_function.deref_mut().deref_mut());
  postscript_function.domain([
    properties.domain.left(),
    properties.domain.right(),
    properties.domain.top(),
    properties.domain.bottom(),
  ]);

  if use_opacities {
    postscript_function.range([0.0, 1.0]);
  } else {
    postscript_function.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
  }

  root_ref
}

/// Postscript code that, given an arbitrary x coordinate, normalizes it to an x coordinate
/// between min and max that yields the correct color, depending on the spread mode. In the case
/// of the `Pad` spread methods, the coordinate will not be normalized since the Postscript functions
/// assign the correct value by default.
fn encode_spread_method<'a>(
  min: f32,
  max: f32,
  code: &mut Vec<PostScriptOp<'a>>,
  bump: &'a Bump,
  spread_method: SpreadMethod,
) {
  use pdf_writer::types::PostScriptOp::*;

  if spread_method == SpreadMethod::Pad {
    return;
  }

  let length = max - min;

  code.extend([
    // We do the following:
    // 1. Normalize by doing n = x - min.
    // 2. Calculate the "interval" we are in by doing i = floor(n / length)
    // 3. Calculate the offset by doing o = n - i * length
    // 4. If the spread method is repeat, we additionally calculate o = length - 0 if i % 2 == 1
    // 5. Calculate the final value with x_new = min + o.

    // Current stack:
    // x
    Real(length),
    Real(min),
    // x length min
    Integer(2),
    Index,
    // x length min x
    Integer(1),
    Index,
    // x length min x min
    Sub,
    // x length min n
    Dup,
    // x length min n n
    Integer(3),
    Index,
    // x length min n n length
    Div,
    // x length min n {n/length}
    Floor,
    // x length min n i
    Exch,
    // x length min i n
    Integer(1),
    Index,
    // x length min i n i
    Integer(4),
    Index,
    // x length min i n i length
    Mul,
    // x length min i n {i * length}
    Sub,
    // x length min i o
    Exch,
    // x length min o i
    Cvi,
    Abs,
    // x length min o abs(i)
    Integer(2),
    Mod,
    // x length min o {abs(i) % 2}
    // See https://github.com/google/skia/blob/645b77ce61449951cb9f3cf754b47d4977b68e1a/src/pdf/SkPDFGradientShader.cpp#L402-L408
    // for why we check > 0 instead of == 1.
    Integer(0),
    Gt,
    // x length min o {(abs(i) % 2) > 0}
    if spread_method == SpreadMethod::Reflect {
      If(bump.alloc([Integer(2), Index, Exch, Sub]))
    } else {
      Pop
    },
    // x length min o
    Add,
    // x length x_new
    Integer(3),
    Integer(1),
    Roll,
    // x_new x length
    Pop,
    Pop,
    // x_new
  ]);
}

/// Postscript code that, given an x coordinate between the min and max
/// of a gradient, returns the interpolated color value depending on where it
/// lies within the stops.
fn encode_postscript_stops<'a>(
  stops: &[Stop],
  min: f32,
  max: f32,
  code: &mut Vec<PostScriptOp<'a>>,
  bump: &'a Bump,
  use_opacities: bool,
) {
  // Our algorithm requires the stops to be padded.
  let mut stops = stops.to_vec();

  // Most viewers have a nesting depth on how many `elseif` can be nested (100 for mupdf,
  // 128 for Chrome and 256 for Acrobat), so we trim the stops if they are too large.
  // this is of course a bit unfortunate, but it should be pretty rare to have that many
  // stops, and if we do, the stops are most likely going to be very similar, so it's
  // safe to just sample from in-between.
  if stops.len() > MAX_POSTSCRIPT_STOPS {
    stops = trim_stops(&stops);
  }

  if let Some(first) = stops.first() {
    let mut first = first.clone();
    first.offset = NormalizedF32::ZERO;
    stops.insert(0, first);
  }

  if let Some(last) = stops.last() {
    let mut last = last.clone();
    last.offset = NormalizedF32::ONE;
    stops.push(last);
  }

  encode_stops_impl(&stops, min, max, code, bump, use_opacities);
}

fn encode_stops_impl<'a>(
  stops: &[Stop],
  min: f32,
  max: f32,
  code: &mut Vec<PostScriptOp<'a>>,
  bump: &'a Bump,
  use_opacities: bool,
) {
  use pdf_writer::types::PostScriptOp::*;

  let encode_two_stops =
    |c0: &[f32], c1: &[f32], min: f32, max: f32, code: &mut Vec<PostScriptOp>| {
      if min == max {
        code.push(Pop);
        code.extend(c0.iter().map(|n| Real(*n)));
        return;
      }

      // Sanity check that both stops have the same number of components.
      assert_eq!(
        c0.len(),
        c1.len(),
        "cannot create gradient with stops from different color spaces"
      );

      // Normalize the x coordinate to be between 0 and 1.
      code.extend([Real(min), Sub, Real(max), Real(min), Sub, Div]);

      for i in 0..c0.len() {
        // Interpolate each color component c0 + x_norm * (x1 - c0).
        code.extend([
          Integer(i as i32),
          Index,
          Real(c0[i]),
          Exch,
          Real(c1[i]),
          Real(c0[i]),
          Sub,
          Mul,
          Add,
        ]);
        // x_norm, c0, c1, ...
      }
      // Remove x_norm from the stack.
      code.extend([Integer((c0.len() + 1) as i32), Integer(-1), Roll, Pop]);
      // c0, c1, c2, ...
    };

  if stops.len() == 1 {
    if use_opacities {
      code.push(Real(stops[0].opacity.get()));
    } else {
      code.extend(stops[0].color.to_pdf_color().into_iter().map(Real));
    }
  } else {
    let length = max - min;
    let stops_min = min + length * stops[0].offset.get();
    let stops_max = min + length * stops[1].offset.get();
    // Write the if conditions to find the corresponding set of two stops.

    let if_stops = bump.alloc(vec![]);
    if use_opacities {
      encode_two_stops(
        &[stops[0].opacity.get()],
        &[stops[1].opacity.get()],
        stops_min,
        stops_max,
        if_stops,
      )
    } else {
      encode_two_stops(
        &stops[0]
          .color
          .to_pdf_color()
          .into_iter()
          .collect::<Vec<_>>(),
        &stops[1]
          .color
          .to_pdf_color()
          .into_iter()
          .collect::<Vec<_>>(),
        stops_min,
        stops_max,
        if_stops,
      )
    };
    let else_stops = bump.alloc(vec![]);
    encode_stops_impl(&stops[1..], min, max, else_stops, bump, use_opacities);

    code.extend([Dup, Real(stops_max), Le, IfElse(if_stops, else_stops)]);
  }
}

fn serialize_stitching(
  stops: &[Stop],
  chunk: &mut Chunk,
  sc: &mut SerializeContext,
  use_opacities: bool,
) -> Ref {
  let root_ref = sc.new_ref();
  let mut functions = vec![];
  let mut bounds = vec![];
  let mut encode = vec![];
  let mut count = 0;

  for window in stops.windows(2) {
    let (first, second) = (&window[0], &window[1]);
    bounds.push(second.offset.get());

    let (c0_components, c1_components) = if use_opacities {
      (vec![first.opacity.get()], vec![second.opacity.get()])
    } else {
      (
        first.color.to_pdf_color().into_iter().collect::<Vec<_>>(),
        second.color.to_pdf_color().into_iter().collect::<Vec<_>>(),
      )
    };

    count = c0_components.len();

    let exp_ref = serialize_exponential(c0_components, c1_components, chunk, sc);

    functions.push(exp_ref);
    encode.extend([0.0, 1.0]);
  }

  bounds.pop();
  let mut stitching_function = chunk.stitching_function(root_ref);
  stitching_function.domain([0.0, 1.0]);
  stitching_function.range([0.0, 1.0].repeat(count));
  stitching_function.functions(functions);
  stitching_function.bounds(bounds);
  stitching_function.encode(encode);

  root_ref
}

fn serialize_exponential(
  c0: Vec<f32>,
  c1: Vec<f32>,
  chunk: &mut Chunk,
  sc: &mut SerializeContext,
) -> Ref {
  let root_ref = sc.new_ref();
  assert_eq!(
    c0.len(),
    c1.len(),
    "cannot create gradient with stops from different color spaces"
  );
  let num_components = c0.len();

  let mut exp = chunk.exponential_function(root_ref);

  exp.range([0.0, 1.0].repeat(num_components));
  exp.c0(c0);
  exp.c1(c1);
  exp.domain([0.0, 1.0]);
  exp.n(1.0);
  exp.finish();
  root_ref
}
