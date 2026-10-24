//! Filters, after Blink's `SVGFilterBuilder` and `FilterEffectBuilder`: the `filter` property of an
//! element becomes the filters its `url()` names and the ones its functions stand for, each with
//! its region and primitives resolved to user space.

use std::{collections::HashSet, sync::Arc};

use super::{
  Scope, Styled, SvgLayout,
  object::{Container, Units},
  painter::SvgPainter,
  view_box::PreserveAspectRatio,
};
use crate::{
  filter::{ColorMatrix, ColorMatrixChain, TransferFunction},
  geometry::{Rect, Size},
  resources::image::SvgOp,
  style::{Angle, BlendMode, Color, ColorInterpolation, ComputedStyle, Filter, FromCssStr, Length},
  svg::{
    approx_eq::ApproxEq,
    element::SvgElementKind,
    filter::{
      ColorChannel, ColorSpace, CompositeOperator, ConvolveMatrix, DiffuseLighting,
      DisplacementMap, DropShadow, EdgeMode, Input, Kernel, Kind, LightSource, MorphologyOperator,
      Primitive, SpecularLighting, SpotLight, SvgFilter, Turbulence,
    },
    length::{LengthAxis, SvgLengthContext},
  },
};

/// Most octaves of noise a turbulence runs, takumi's own limit past which one adds nothing to a pixel.
const MAX_OCTAVES: f32 = 24.0;

impl SvgLayout<'_> {
  /// The filter the element `id` names, for content of `width` by `height` at the origin.
  pub(in crate::svg) fn filter_by_id(
    mut self,
    id: &str,
    width: f32,
    height: f32,
  ) -> Option<Arc<SvgFilter>> {
    let link = *self.document.last_ids.get(id)?;
    let bounds = Rect::positive_from_xywh(0.0, 0.0, width, height)?;

    self.filter_element(link, &Scope::root(bounds), Some(bounds))
  }

  /// The filters an element draws through, none when one of them names no filter and nothing else
  /// is left, which hides the element.
  pub(super) fn convert_filters(
    &mut self,
    styled: &Styled,
    scope: &Scope,
    bounding_box: Option<Rect<f32>>,
  ) -> Option<Vec<Arc<SvgFilter>>> {
    let mut filters = Vec::new();
    let mut names_nothing = false;
    // Consecutive color functions fold into one pass.
    let mut colors = ColorMatrixChain::default();

    for filter in &styled.style().misc1_data.filter {
      if let Some(matrix) = ColorMatrix::from_filter(filter) {
        colors.push(matrix);
        continue;
      }

      if !colors.0.is_empty() {
        let chain = std::mem::take(&mut colors);

        filters.extend(self.function_filter(
          Kind::ColorMatrix {
            input: Input::SourceGraphic,
            chain,
          },
          bounding_box,
        ));
      }

      match filter {
        Filter::Url(resource) => {
          let link = self.reference(resource);

          // An element that a filter of its own lays out with `feImage` goes unfiltered by it.
          if link.is_some_and(|link| self.draws_element_in_image(link, styled.index)) {
            continue;
          }

          let element = link.and_then(|link| self.filter_element(link, scope, bounding_box));

          match element {
            Some(element) => filters.push(element),
            None => names_nothing = true,
          }
        }
        Filter::Reference(_) => {}
        function => {
          let kind = self.function_kind(function, styled, scope);

          filters.extend(kind.and_then(|kind| self.function_filter(kind, bounding_box)));
        }
      }
    }

    if !colors.0.is_empty() {
      filters.extend(self.function_filter(
        Kind::ColorMatrix {
          input: Input::SourceGraphic,
          chain: colors,
        },
        bounding_box,
      ));
    }

    (!filters.is_empty() || !names_nothing).then_some(filters)
  }

  /// Whether a primitive of the filter at `filter` is an `feImage` that draws `element`.
  fn draws_element_in_image(&self, filter: usize, element: usize) -> bool {
    self
      .document
      .element_children(filter)
      .into_iter()
      .any(|child| {
        self.styles.node(child).tag_name() == Some("feImage")
          && self.href_target(child) == Some(element)
      })
  }

  /// The filter a CSS filter function stands for, none for one without a region to filter.
  fn function_filter(&self, kind: Kind, bounding_box: Option<Rect<f32>>) -> Option<Arc<SvgFilter>> {
    // A function has no filter region, so it takes a fairly large one.
    let region = match kind {
      Kind::DropShadow(_) | Kind::GaussianBlur { .. } => {
        Rect::positive_from_xywh(-0.5, -0.5, 2.0, 2.0)
      }
      _ => Rect::positive_from_xywh(-0.1, -0.1, 1.2, 1.2),
    }?
    .in_bounding_box(&bounding_box?)?;

    Some(Arc::new(SvgFilter {
      region,
      primitives: vec![Primitive {
        subregion: region,
        // A function, unlike a `filter` element, computes in sRGB.
        color_space: ColorSpace::Srgb,
        result: "result".to_owned(),
        kind,
      }],
    }))
  }

  /// The primitive a function stands for when it is not a color one.
  fn function_kind(&self, function: &Filter, styled: &Styled, scope: &Scope) -> Option<Kind> {
    let input = Input::SourceGraphic;

    Some(match function {
      Filter::Blur(length) => {
        let std_dev = self.px(styled, scope, *length, LengthAxis::X).max(0.0);

        Kind::GaussianBlur {
          input,
          std_dev_x: std_dev,
          std_dev_y: std_dev,
        }
      }
      Filter::DropShadow(shadow) => {
        let Color([red, green, blue, alpha]) = shadow.color.resolve(styled.context.current_color);

        Kind::DropShadow(DropShadow {
          input,
          dx: self.px(styled, scope, shadow.offset_x, LengthAxis::X),
          dy: self.px(styled, scope, shadow.offset_y, LengthAxis::Y),
          std_dev_x: self
            .px(styled, scope, shadow.blur_radius, LengthAxis::X)
            .max(0.0),
          std_dev_y: self
            .px(styled, scope, shadow.blur_radius, LengthAxis::X)
            .max(0.0),
          color: Color([red, green, blue, 255]),
          opacity: f32::from(alpha) / 255.0,
        })
      }
      _ => return None,
    })
  }

  /// The filter the element at `link` makes for content with `bounding_box`, none when it is not
  /// a valid filter. A filter in user units is shared by every element it filters.
  fn filter_element(
    &mut self,
    link: usize,
    scope: &Scope,
    bounding_box: Option<Rect<f32>>,
  ) -> Option<Arc<SvgFilter>> {
    if self.document.elements[link].kind != SvgElementKind::Filter {
      return None;
    }

    let units = self.units(link, "filterUnits", Units::ObjectBoundingBox);
    let primitive_units = self.units(link, "primitiveUnits", Units::UserSpaceOnUse);
    let key = (link, scope.view_box.to_bits());
    let cacheable = units == Units::UserSpaceOnUse && primitive_units == Units::UserSpaceOnUse;

    if cacheable && let Some(filter) = self.filters.get(&key) {
      return Some(Arc::clone(filter));
    }

    let filter = self
      .guarded(link, |layout| {
        layout.build_filter(link, units, primitive_units, scope, bounding_box)
      })
      .flatten()?;

    if cacheable {
      self.filters.insert(key, Arc::clone(&filter));
    }

    Some(filter)
  }

  fn build_filter(
    &mut self,
    link: usize,
    units: Units,
    primitive_units: Units,
    scope: &Scope,
    bounding_box: Option<Rect<f32>>,
  ) -> Option<Arc<SvgFilter>> {
    let number = |layout: &Self, name: &str, percent: f32| {
      layout.resolve_number(link, name, units, scope, Length::Percentage(percent))
    };
    let mut region = Rect::positive_from_xywh(
      number(self, "x", -10.0),
      number(self, "y", -10.0),
      number(self, "width", 120.0),
      number(self, "height", 120.0),
    )?;

    if units == Units::ObjectBoundingBox {
      region = region.in_bounding_box(&bounding_box?)?;
    }

    let source = self.filter_with_primitives(link)?;
    let primitives = self.primitives(source, primitive_units, scope, bounding_box, region);

    (!primitives.is_empty()).then(|| Arc::new(SvgFilter { region, primitives }))
  }

  /// The element of the `href` chain of the filter that has primitives, none when the chain meets
  /// anything but a filter first.
  fn filter_with_primitives(&self, link: usize) -> Option<usize> {
    for element in self.href_chain(link) {
      if self.document.elements[element].kind != SvgElementKind::Filter {
        return None;
      }

      if !self.document.element_children(element).is_empty() {
        return Some(element);
      }
    }

    None
  }

  fn primitives(
    &mut self,
    filter: usize,
    units: Units,
    scope: &Scope,
    bounding_box: Option<Rect<f32>>,
    region: Rect<f32>,
  ) -> Vec<Primitive> {
    let scale = match (units, bounding_box) {
      (Units::ObjectBoundingBox, Some(bounds)) => Size::new(bounds.width(), bounds.height()),
      (Units::ObjectBoundingBox, None) => return Vec::new(),
      (Units::UserSpaceOnUse, _) => Size::new(1.0, 1.0),
    };
    let mut primitives: Vec<Primitive> = Vec::new();
    let mut names = HashSet::new();
    let mut counter = 1;

    for child in self.document.element_children(filter) {
      if self.document.elements[child].kind != SvgElementKind::FilterPrimitive {
        continue;
      }

      let tag = self.styles.node(child).tag_name().unwrap_or_default();
      let Some(subregion) =
        self.primitive_subregion(child, tag, units, scope, bounding_box, region)
      else {
        break;
      };
      let Some(kind) = self.primitive_kind(child, tag, scale, subregion, scope, &primitives) else {
        continue;
      };
      let color_space = match self
        .styled_in_place(child)
        .style()
        .svg_inherited_data
        .color_interpolation_filters
      {
        ColorInterpolation::LinearRgb => ColorSpace::LinearRgb,
        ColorInterpolation::Srgb | ColorInterpolation::Auto => ColorSpace::Srgb,
      };

      primitives.push(Primitive {
        subregion,
        color_space,
        result: self.result_name(child, &mut names, &mut counter),
        kind,
      });
    }

    primitives
  }

  /// The name later primitives read the result of the primitive by.
  fn result_name(&self, child: usize, names: &mut HashSet<String>, counter: &mut usize) -> String {
    if let Some(name) = self.attribute(child, "result") {
      names.insert(name.to_owned());
      *counter += 1;

      return name.to_owned();
    }

    loop {
      let name = format!("result{counter}");

      *counter += 1;

      if !names.contains(&name) {
        return name;
      }
    }
  }

  /// The subregion of a primitive, none when it is invalid.
  fn primitive_subregion(
    &self,
    child: usize,
    tag: &str,
    units: Units,
    scope: &Scope,
    bounding_box: Option<Rect<f32>>,
    region: Rect<f32>,
  ) -> Option<Rect<f32>> {
    let styled = self.styled_in_place(child);
    let length = |name: &str, axis| {
      let length = self.length_attribute(child, name)?;

      Some(if units == Units::ObjectBoundingBox {
        SvgLengthContext::resolve_in_bounding_box(length, styled.sizing())
      } else {
        self.px(&styled, scope, length, axis)
      })
    };
    let (x, y) = (length("x", LengthAxis::X), length("y", LengthAxis::Y));
    let (width, height) = (
      length("width", LengthAxis::X),
      length("height", LengthAxis::Y),
    );
    let fractions = || {
      Rect::positive_from_xywh(
        x.unwrap_or(0.0),
        y.unwrap_or(0.0),
        width.unwrap_or(1.0),
        height.unwrap_or(1.0),
      )
    };

    // `feFlood` and `feImage` take their fractions from the bounding box.
    if units == Units::ObjectBoundingBox && matches!(tag, "feFlood" | "feImage") {
      return fractions()?.in_bounding_box(&bounding_box?);
    }

    if units == Units::ObjectBoundingBox {
      return region.in_bounding_box(&fractions()?);
    }

    Rect::positive_from_xywh(
      x.unwrap_or(region.x()),
      y.unwrap_or(region.y()),
      width.unwrap_or(region.width()),
      height.unwrap_or(region.height()),
    )
  }

  /// A primitive that is malformed makes a transparent image, which a clear flood stands for.
  fn dummy_primitive() -> Kind {
    Kind::Flood {
      color: Color::black(),
      opacity: 0.0,
    }
  }

  /// What a primitive reads: the input the attribute names, else the result before it.
  fn input(&self, child: usize, name: &str, primitives: &[Primitive]) -> Input {
    let previous = || {
      primitives.last().map_or(Input::SourceGraphic, |last| {
        Input::Reference(last.result.clone())
      })
    };

    match self.attribute(child, name) {
      Some("SourceGraphic") => Input::SourceGraphic,
      Some("SourceAlpha") => Input::SourceAlpha,
      // The inputs that need a backdrop or a paint are not supported.
      Some("BackgroundImage" | "BackgroundAlpha" | "FillPaint" | "StrokePaint") => {
        Input::SourceGraphic
      }
      // A result nothing made falls back to the result before.
      Some(reference)
        if primitives
          .iter()
          .any(|primitive| primitive.result == reference) =>
      {
        Input::Reference(reference.to_owned())
      }
      _ => previous(),
    }
  }

  fn primitive_kind(
    &mut self,
    fe: usize,
    tag: &str,
    scale: Size<f32>,
    subregion: Rect<f32>,
    scope: &Scope,
    primitives: &[Primitive],
  ) -> Option<Kind> {
    let input = |layout: &Self, name: &str| layout.input(fe, name, primitives);
    let number = |name: &str| self.number_attribute(fe, name);
    let styled = self.styled_in_place(fe);
    let style = styled.style();

    Some(match tag {
      "feDropShadow" => {
        let (std_dev_x, std_dev_y) = self.std_deviation(fe, scale, "2 2");
        let (color, opacity) = flood(&styled, style);

        Kind::DropShadow(DropShadow {
          input: input(self, "in"),
          dx: number("dx").unwrap_or(2.0) * scale.width,
          dy: number("dy").unwrap_or(2.0) * scale.height,
          std_dev_x,
          std_dev_y,
          color,
          opacity,
        })
      }
      "feGaussianBlur" => {
        let (std_dev_x, std_dev_y) = self.std_deviation(fe, scale, "0 0");

        Kind::GaussianBlur {
          input: input(self, "in"),
          std_dev_x,
          std_dev_y,
        }
      }
      "feOffset" => Kind::Offset {
        input: input(self, "in"),
        dx: number("dx").unwrap_or(0.0) * scale.width,
        dy: number("dy").unwrap_or(0.0) * scale.height,
      },
      "feBlend" => Kind::Blend {
        input1: input(self, "in"),
        input2: input(self, "in2"),
        mode: self
          .attribute(fe, "mode")
          .and_then(|mode| BlendMode::from_css_str(mode).ok())
          .unwrap_or_default(),
      },
      "feFlood" => {
        let (color, opacity) = flood(&styled, style);

        Kind::Flood { color, opacity }
      }
      "feComposite" => Kind::Composite {
        input1: input(self, "in"),
        input2: input(self, "in2"),
        operator: self.composite_operator(fe),
      },
      "feMerge" => Kind::Merge {
        inputs: self
          .document
          .element_children(fe)
          .into_iter()
          .map(|child| self.input(child, "in", primitives))
          .collect(),
      },
      "feTile" => Kind::Tile {
        input: input(self, "in"),
      },
      "feImage" => self.image_primitive(fe, subregion, scope),
      "feComponentTransfer" => Kind::ComponentTransfer {
        input: input(self, "in"),
        tables: Box::new(self.transfer_functions(fe).map(|function| function.table())),
      },
      "feColorMatrix" => Kind::ColorMatrix {
        input: input(self, "in"),
        chain: self.color_matrix(fe).unwrap_or_default().into(),
      },
      "feConvolveMatrix" => self
        .convolve_matrix(fe, input(self, "in"))
        .unwrap_or_else(Self::dummy_primitive),
      "feMorphology" => self.morphology(fe, scale, input(self, "in")),
      "feDisplacementMap" => {
        let channel = |name: &str| match self.attribute(fe, name) {
          Some("R") => ColorChannel::R,
          Some("G") => ColorChannel::G,
          Some("B") => ColorChannel::B,
          _ => ColorChannel::A,
        };

        Kind::DisplacementMap(DisplacementMap {
          input1: input(self, "in"),
          input2: input(self, "in2"),
          scale: number("scale").unwrap_or(0.0) * (scale.width + scale.height) / 2.0,
          x_channel: channel("xChannelSelector"),
          y_channel: channel("yChannelSelector"),
        })
      }
      "feTurbulence" => self.turbulence(fe),
      "feDiffuseLighting" => self
        .light_source(fe)
        .map(|light_source| {
          Kind::DiffuseLighting(DiffuseLighting {
            input: input(self, "in"),
            surface_scale: number("surfaceScale").unwrap_or(1.0),
            diffuse_constant: number("diffuseConstant").unwrap_or(1.0),
            lighting_color: lighting_color(&styled, style),
            light_source,
          })
        })
        .unwrap_or_else(Self::dummy_primitive),
      "feSpecularLighting" => {
        let exponent = number("specularExponent").unwrap_or(1.0);

        // An exponent out of range ignores the primitive.
        match self.light_source(fe) {
          Some(light_source) if (1.0..=128.0).contains(&exponent) => {
            Kind::SpecularLighting(SpecularLighting {
              input: input(self, "in"),
              surface_scale: number("surfaceScale").unwrap_or(1.0),
              specular_constant: number("specularConstant").unwrap_or(1.0),
              specular_exponent: exponent,
              lighting_color: lighting_color(&styled, style),
              light_source,
            })
          }
          _ => Self::dummy_primitive(),
        }
      }
      _ => return None,
    })
  }

  /// `stdDeviation` as two numbers, `0 0` unless it has one or two.
  fn std_deviation(&self, fe: usize, scale: Size<f32>, default: &str) -> (f32, f32) {
    let numbers = self
      .attribute(fe, "stdDeviation")
      .and_then(|_| self.number_list(fe, "stdDeviation"))
      .or_else(|| {
        default
          .split_ascii_whitespace()
          .map(|number| number.parse().ok())
          .collect()
      })
      .unwrap_or_default();
    let (x, y) = match numbers.as_slice() {
      [x, y] => (*x, *y),
      [x] => (*x, *x),
      _ => (0.0, 0.0),
    };

    ((x * scale.width).max(0.0), (y * scale.height).max(0.0))
  }

  fn composite_operator(&self, fe: usize) -> CompositeOperator {
    let k = |name: &str| self.number_attribute(fe, name).unwrap_or(0.0);

    match self.attribute(fe, "operator").unwrap_or("over") {
      "in" => CompositeOperator::In,
      "out" => CompositeOperator::Out,
      "atop" => CompositeOperator::Atop,
      "xor" => CompositeOperator::Xor,
      "arithmetic" => CompositeOperator::Arithmetic {
        k1: k("k1"),
        k2: k("k2"),
        k3: k("k3"),
        k4: k("k4"),
      },
      _ => CompositeOperator::Over,
    }
  }

  fn color_matrix(&self, fe: usize) -> Option<ColorMatrix> {
    let values = self.number_list(fe, "values");

    match self.attribute(fe, "type") {
      Some("saturate") => Some(ColorMatrix::saturate(
        values?.first().map_or(1.0, |value| value.clamp(0.0, 1.0)),
      )),
      Some("hueRotate") => Some(ColorMatrix::hue_rotate(Angle::new(
        values?.first().copied().unwrap_or(0.0),
      ))),
      Some("luminanceToAlpha") => Some(ColorMatrix::luminance_to_alpha()),
      _ => values
        .and_then(|list| <[f32; 20]>::try_from(list).ok())
        .map(ColorMatrix::from_values),
    }
  }

  fn transfer_functions(&self, fe: usize) -> [TransferFunction; 4] {
    let mut functions = [
      TransferFunction::Identity,
      TransferFunction::Identity,
      TransferFunction::Identity,
      TransferFunction::Identity,
    ];

    for child in self.document.element_children(fe) {
      let slot = match self.styles.node(child).tag_name() {
        Some("feFuncR") => 0,
        Some("feFuncG") => 1,
        Some("feFuncB") => 2,
        Some("feFuncA") => 3,
        _ => continue,
      };

      if let Some(function) = self.transfer_function(child) {
        functions[slot] = function;
      }
    }

    functions
  }

  fn transfer_function(&self, node: usize) -> Option<TransferFunction> {
    let number = |name: &str, default: f32| self.number_attribute(node, name).unwrap_or(default);
    let table = || self.number_list(node, "tableValues").unwrap_or_default();

    match self.attribute(node, "type")? {
      "identity" => Some(TransferFunction::Identity),
      "table" => Some(TransferFunction::Table(table())),
      "discrete" => Some(TransferFunction::Discrete(table())),
      "linear" => Some(TransferFunction::Linear {
        slope: number("slope", 1.0),
        intercept: number("intercept", 0.0),
      }),
      "gamma" => Some(TransferFunction::Gamma {
        amplitude: number("amplitude", 1.0),
        exponent: number("exponent", 1.0),
        offset: number("offset", 0.0),
      }),
      _ => None,
    }
  }

  fn convolve_matrix(&self, fe: usize, input: Input) -> Option<Kind> {
    let parse_target = |target: Option<f32>, order: u32| {
      let target = target.unwrap_or((order as f32 / 2.0).floor()) as i32;

      u32::try_from(target).ok().filter(|target| *target < order)
    };
    let (mut order_x, mut order_y) = (3, 3);

    if let Some(order) = self.number_list(fe, "order") {
      let x = order.first().map_or(3, |value| *value as i32);
      let y = order.get(1).map_or(x, |value| *value as i32);

      if x > 0 && y > 0 {
        (order_x, order_y) = (x as u32, y as u32);
      }
    }

    let matrix = self
      .number_list(fe, "kernelMatrix")
      .filter(|list| order_x.checked_mul(order_y).map(|len| len as usize) == Some(list.len()))
      .unwrap_or_default();
    // The sum rounds up to keep float precision from deciding it.
    let mut kernel_sum = (matrix.iter().sum::<f32>() * 1_000_000.0).round() / 1_000_000.0;

    if approx_zero(kernel_sum) {
      kernel_sum = 1.0;
    }

    let divisor = self.number_attribute(fe, "divisor").unwrap_or(kernel_sum);

    if approx_zero(divisor) {
      return None;
    }

    let kernel = Kernel::new(
      parse_target(self.number_attribute(fe, "targetX"), order_x)?,
      parse_target(self.number_attribute(fe, "targetY"), order_y)?,
      order_x,
      order_y,
      matrix,
    )?;

    Some(Kind::ConvolveMatrix(ConvolveMatrix {
      input,
      kernel,
      divisor,
      bias: self.number_attribute(fe, "bias").unwrap_or(0.0),
      edge_mode: match self.attribute(fe, "edgeMode").unwrap_or("duplicate") {
        "none" => EdgeMode::None,
        "wrap" => EdgeMode::Wrap,
        _ => EdgeMode::Duplicate,
      },
      preserve_alpha: self.attribute(fe, "preserveAlpha") == Some("true"),
    }))
  }

  fn morphology(&self, fe: usize, scale: Size<f32>, input: Input) -> Kind {
    let operator = match self.attribute(fe, "operator").unwrap_or("erode") {
      "dilate" => MorphologyOperator::Dilate,
      _ => MorphologyOperator::Erode,
    };
    let (mut radius_x, mut radius_y) = (scale.width, scale.height);

    if let Some(list) = self.number_list(fe, "radius") {
      let (mut x, mut y) = match list.as_slice() {
        [x, y] => (*x, *y),
        [x] => (*x, *x),
        _ => (0.0, 0.0),
      };

      if approx_zero(x) && approx_zero(y) {
        (x, y) = (1.0, 1.0);
      }

      // One radius of zero becomes one, as Chrome and Safari do.
      if approx_zero(x) && !approx_zero(y) {
        x = 1.0;
      }

      if !approx_zero(x) && approx_zero(y) {
        y = 1.0;
      }

      // Both radii must be positive.
      if x.is_sign_positive() && y.is_sign_positive() {
        (radius_x, radius_y) = (x * scale.width, y * scale.height);
      }
    }

    Kind::Morphology {
      input,
      operator,
      radius_x,
      radius_y,
    }
  }

  fn turbulence(&self, fe: usize) -> Kind {
    let (mut base_frequency_x, mut base_frequency_y) = (0.0, 0.0);

    if let Some(list) = self.number_list(fe, "baseFrequency") {
      let (x, y) = match list.as_slice() {
        [x, y] => (*x, *y),
        [x] => (*x, *x),
        _ => (0.0, 0.0),
      };

      if x.is_sign_positive() && y.is_sign_positive() {
        (base_frequency_x, base_frequency_y) = (x, y);
      }
    }

    let octaves = self.number_attribute(fe, "numOctaves").unwrap_or(1.0);

    Kind::Turbulence(Turbulence {
      base_frequency_x,
      base_frequency_y,
      num_octaves: octaves.clamp(0.0, MAX_OCTAVES).round() as u32,
      seed: self.number_attribute(fe, "seed").unwrap_or(0.0).trunc() as i32,
      stitch_tiles: self.attribute(fe, "stitchTiles") == Some("stitch"),
      fractal_noise: self.attribute(fe, "type") == Some("fractalNoise"),
    })
  }

  /// The first light source child of a lighting primitive.
  fn light_source(&self, parent: usize) -> Option<LightSource> {
    let child = self
      .document
      .element_children(parent)
      .into_iter()
      .find(|child| {
        matches!(
          self.styles.node(*child).tag_name(),
          Some("feDistantLight" | "fePointLight" | "feSpotLight")
        )
      })?;
    let number = |name: &str| self.number_attribute(child, name).unwrap_or(0.0);

    match self.styles.node(child).tag_name()? {
      "feDistantLight" => Some(LightSource::Distant {
        azimuth: number("azimuth"),
        elevation: number("elevation"),
      }),
      "fePointLight" => Some(LightSource::Point {
        x: number("x"),
        y: number("y"),
        z: number("z"),
      }),
      _ => Some(LightSource::Spot(SpotLight {
        x: number("x"),
        y: number("y"),
        z: number("z"),
        points_at_x: number("pointsAtX"),
        points_at_y: number("pointsAtY"),
        points_at_z: number("pointsAtZ"),
        specular_exponent: self
          .number_attribute(child, "specularExponent")
          .filter(|exponent| *exponent >= 0.0)
          .unwrap_or(1.0),
        limiting_cone_angle: self.number_attribute(child, "limitingConeAngle"),
      })),
    }
  }

  /// The ops of the element or image an `feImage` names, laid out at the origin.
  fn image_primitive(&mut self, fe: usize, subregion: Rect<f32>, scope: &Scope) -> Kind {
    let Some(ops) = self.image_ops(fe, subregion, scope) else {
      return Self::dummy_primitive();
    };

    Kind::Image(ops)
  }

  fn image_ops(&mut self, fe: usize, subregion: Rect<f32>, scope: &Scope) -> Option<Vec<SvgOp>> {
    let mut root = Container::new(crate::style::Affine::IDENTITY);

    if let Some(target) = self.href_target(fe) {
      let parent_context = match self.document.elements[target].parent {
        Some(parent) => self.styles.in_place(parent),
        None => self.styles.root_context().clone(),
      };

      self.convert_element(target, &parent_context, scope, &mut root);
    } else {
      let (content, size) = self.image_content(self.attribute(fe, "href")?)?;
      let aspect = self
        .attribute(fe, "preserveAspectRatio")
        .and_then(PreserveAspectRatio::parse)
        .unwrap_or_default();
      let rect = Rect::positive_from_xywh(0.0, 0.0, subregion.width(), subregion.height())?;
      let picture = super::object::Picture {
        content,
        width: size.width,
        height: size.height,
        visible: true,
        rendering: self
          .styled_in_place(fe)
          .style()
          .misc_inherited_data
          .image_rendering,
      };

      Self::fit_image(picture, aspect, size, rect, &mut root)?;
    }

    if !root.has_children() {
      return None;
    }

    root.finish();
    SvgPainter::paint(&root, &mut self.budget)
  }
}

/// The color and opacity of the flood a primitive makes, from `flood-color` and `flood-opacity`.
fn flood(styled: &Styled, style: &ComputedStyle) -> (Color, f32) {
  let Color([red, green, blue, alpha]) = style
    .svg_misc_data
    .flood_color
    .resolve(styled.context.current_color);

  (
    Color([red, green, blue, 255]),
    f32::from(alpha) / 255.0 * style.svg_misc_data.flood_opacity.0.clamp(0.0, 1.0),
  )
}

/// The color of a lighting primitive, whose alpha does not matter.
fn lighting_color(styled: &Styled, style: &ComputedStyle) -> Color {
  let Color([red, green, blue, _]) = style
    .svg_misc_data
    .lighting_color
    .resolve(styled.context.current_color);

  Color([red, green, blue, 255])
}

fn approx_zero(value: f32) -> bool {
  value.approx_eq(0.0)
}
