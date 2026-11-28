// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Running the primitives of a filter on a layer. The structure is resvg's `filter/mod.rs`, with
//! Blink's `FilterEffect` in `filter_effect.cc` as the model for what each primitive reads.

use std::{f32::consts::SQRT_2, rc::Rc};

use crate::{
  blend::unorm_to_byte,
  pixmap::{Pixmap, PixmapRef},
};

use super::{
  ColorSpace, CompositeOperator, ConvolveMatrix, DisplacementMap, DropShadow, Input, Kind,
  LightSource, MorphologyOperator, Primitive, SpotLight, SvgFilter, Turbulence,
  color_filter::{ColorFilter, ColorFilters, convert_color},
  composite, convolve_matrix, displacement_map,
  lighting::{self, Lighting},
  morphology, turbulence,
};
use takumi_core::{
  blur::{MIN_SIGMA, blur_rgba_axes},
  geometry::{Point, Rect, Size},
  math::ApproxEq,
  resources::{
    image::{MAX_RASTER_PIXELS, within_raster_pixel_budget},
    image_buffer::premultiply_pixel,
  },
  style::{
    Affine, BlendMode, Color,
    math::{div_255, mul_div_255},
  },
};

use crate::{
  blend::{blend_premultiplied_pixel, composite_premultiplied_over, scale_premultiplied_pixel},
  svg::drawing::{draw as draw_svg, draw_over},
};

/// A primitive's pixels over the filter region, with `filters` deferred as Skia's `FilterResult`
/// defers them.
#[derive(Clone)]
struct Image {
  pixmap: Rc<Pixmap>,
  /// The part of the filter region with data, in the layer's pixels. The rest is transparent.
  region: Rect<i32>,
  /// The space of the colors after `filters`.
  color_space: ColorSpace,
  filters: ColorFilters,
}

impl Image {
  fn new(pixmap: Pixmap, color_space: ColorSpace) -> Self {
    Self {
      region: Rect::from_size(0, 0, pixmap.width(), pixmap.height()),
      pixmap: Rc::new(pixmap),
      color_space,
      filters: ColorFilters::default(),
    }
  }

  /// The image with `filter` deferred after its other filters.
  fn with_filter(mut self, filter: ColorFilter) -> Self {
    self.filters.push(filter);
    self
  }

  /// The image with its colors to be converted to `color_space`, deferred.
  fn in_color_space(self, color_space: ColorSpace) -> Self {
    if color_space == self.color_space {
      return self;
    }

    Self {
      color_space,
      ..self.with_filter(ColorFilter::Space(color_space))
    }
  }

  /// The pixels in `color_space`, with every filter run.
  fn into_color_space(self, color_space: ColorSpace) -> Self {
    self.in_color_space(color_space).resolved()
  }

  /// The image with its filters run over every pixel.
  fn resolved(self) -> Self {
    let (width, height) = (self.pixmap.width() as usize, self.pixmap.height() as usize);

    self.resolved_within(Rect {
      left: 0,
      right: width,
      top: 0,
      bottom: height,
    })
  }

  /// The image with its filters run over the pixels of `window`, leaving the rest as they were.
  fn resolved_within(mut self, window: Rect<usize>) -> Self {
    if self.filters.is_empty() {
      return self;
    }

    let filters = std::mem::take(&mut self.filters);
    let region = self.region;
    let color_space = self.color_space;
    let mut pixmap = Rc::unwrap_or_clone(self.pixmap);
    let width = pixmap.width() as usize;

    for line in
      pixmap.pixels_mut()[window.top * width..window.bottom * width].chunks_exact_mut(width)
    {
      filters.apply(&mut line[window.left..window.right]);
    }

    Self {
      pixmap: Rc::new(pixmap),
      region,
      color_space,
      filters: ColorFilters::default(),
    }
  }

  fn view(&self) -> PixmapRef<'_> {
    Pixmap::as_ref(&self.pixmap)
  }

  /// A transparent pixmap as large as the image.
  fn blank_like(&self) -> Option<Pixmap> {
    Pixmap::new(Size::new(self.pixmap.width(), self.pixmap.height()))
  }

  /// The image with its colors black, as `SourceAlpha` is.
  fn alpha(&self) -> Self {
    let mut pixmap = (*self.pixmap).clone();

    for pixel in pixmap.pixels_mut() {
      pixel[..3].fill(0);
    }

    Self {
      pixmap: Rc::new(pixmap),
      ..self.clone()
    }
  }

  /// The pixels with every filter run, copied only when another result still reads them.
  fn take(self) -> Pixmap {
    Rc::unwrap_or_clone(self.resolved().pixmap)
  }
}

struct FilterResult {
  name: String,
  image: Image,
}

/// What running a filter keeps between its primitives.
struct Run<'a> {
  transform: Affine,
  /// The filter region in the pixels of the layer.
  region: Rect<i32>,
  /// The layer and its alpha, kept until the last primitive that reads them.
  source_graphic: Option<Image>,
  source_alpha: Option<Image>,
  results: Vec<FilterResult>,
  /// The primitives after the one running.
  later: &'a [Primitive],
}

/// Applies the filter to a layer, which `transform` maps the user space of the filter onto. A
/// filter that cannot run clears the layer.
pub(crate) fn apply(filter: &SvgFilter, transform: Affine, layer: &mut Pixmap) {
  let (width, height) = (layer.width(), layer.height());
  let Some(blank) = Pixmap::new(Size::new(width, height)) else {
    return;
  };
  let source = std::mem::replace(layer, blank);

  if let Some((image, region)) = run(filter, transform, source) {
    if region == Rect::from_size(0, 0, width, height) {
      *layer = image.take();
    } else {
      draw(layer, image.view(), region.left, region.top, Layer::Over);
    }
  }
}

/// The output of the primitives on the layer, in sRGB, and the part of the layer it covers.
fn run(filter: &SvgFilter, transform: Affine, source: Pixmap) -> Option<(Image, Rect<i32>)> {
  // The filter region comes straight from the filter, so it can be larger than the layer.
  let layer = Rect::from_size(0, 0, source.width(), source.height());
  let region = filter
    .region
    .transformed(transform)
    .and_then(pixel_bounds)?
    .intersection(&layer)?;
  // Every input of a primitive is as large as the region.
  let source = if region == layer {
    source
  } else {
    source.clone_rect(Rect::checked_from_size(
      region.left,
      region.top,
      region.width(),
      region.height(),
    )?)?
  };
  let mut graphic = Image::new(source, ColorSpace::Srgb);

  graphic.region = region;

  let alpha = filter
    .primitives
    .iter()
    .any(|primitive| primitive.kind.inputs().contains(&&Input::SourceAlpha))
    .then(|| graphic.alpha());
  let mut run = Run {
    transform,
    region,
    source_graphic: Some(graphic),
    source_alpha: alpha,
    results: Vec::new(),
    later: &filter.primitives,
  };

  for (index, primitive) in filter.primitives.iter().enumerate() {
    run.later = &filter.primitives[index + 1..];

    let image = run.primitive(primitive)?;

    run.results.push(FilterResult {
      name: primitive.result.clone(),
      image,
    });

    run.release(true);

    if run.live_bytes() > MAX_LIVE_BYTES {
      return None;
    }
  }

  let image = run.results.pop()?.image;

  Some((image.into_color_space(ColorSpace::Srgb), region))
}

/// Applies the filter to a premultiplied RGBA layer in place, `None` past the raster budget.
pub(crate) fn apply_to_layer(
  filter: &SvgFilter,
  layer: &mut [u8],
  width: u32,
  height: u32,
) -> Option<()> {
  // The pixels of the region with its top-left corner at the origin, which the layer sits within
  // at `dx`, `dy`.
  let dx = (-filter.region.left).round() as i32;
  let dy = (-filter.region.top).round() as i32;
  let window = filter
    .region
    .transformed(Affine::translation(-filter.region.left, -filter.region.top))
    .and_then(pixel_bounds)?
    .intersection(&Rect::from_size(dx, dy, width, height))?;

  if !within_raster_pixel_budget(window.width(), window.height()) {
    return None;
  }

  let mut sub = Pixmap::new(window.size())?;
  let transform = Affine::translation(
    -filter.region.left - window.left as f32,
    -filter.region.top - window.top as f32,
  );

  draw(
    &mut sub,
    PixmapRef::from_bytes(layer, Size::new(width, height))?,
    dx - window.left,
    dy - window.top,
    Layer::Over,
  );
  apply(filter, transform, &mut sub);

  let mut out = Pixmap::new(Size::new(width, height))?;

  draw(
    &mut out,
    sub.as_ref(),
    window.left - dx,
    window.top - dy,
    Layer::Over,
  );
  layer.copy_from_slice(out.data());

  Some(())
}

/// Most bytes a filter's live results and sources hold at once, takumi's own limit.
const MAX_LIVE_BYTES: u64 = MAX_RASTER_PIXELS * 4;

impl Run<'_> {
  /// The bytes of the pixmaps the run keeps.
  fn live_bytes(&self) -> u64 {
    self
      .results
      .iter()
      .map(|result| &result.image)
      .chain(&self.source_graphic)
      .chain(&self.source_alpha)
      .map(|image| image.pixmap.data().len() as u64)
      .sum()
  }

  /// Runs one primitive, cutting its result to its subregion.
  fn primitive(&mut self, primitive: &Primitive) -> Option<Image> {
    let mut subregion = primitive
      .subregion
      .transformed(self.transform)
      .and_then(pixel_bounds)?;

    // `feOffset` inherits its region from the input.
    if let Kind::Offset {
      input: Input::Reference(name),
      ..
    } = &primitive.kind
      && let Some(result) = self
        .results
        .iter()
        .rev()
        .find(|result| result.name == *name)
    {
      subregion = result.image.region;
    }

    let mut image = self.apply_kind(primitive, subregion)?;

    if self.region != subregion {
      let color_space = image.color_space;
      let mut pixmap = image.take();

      // feOffset is not clipped.
      if !matches!(primitive.kind, Kind::Offset { .. }) {
        clear_outside(
          &mut pixmap,
          subregion.translated(-self.region.left, -self.region.top),
        );
      }

      image = Image::new(pixmap, color_space);
    }

    image.region = subregion;

    Some(image)
  }

  /// Drops the results no later primitive reads, so a last reader takes its pixels without a copy.
  fn release(&mut self, keep_last: bool) {
    let last = self.results.len().saturating_sub(1);
    let later = self.later;
    let mut index = 0;
    let read_later = |input: &Input| {
      later
        .iter()
        .any(|primitive| primitive.kind.inputs().contains(&input))
    };

    if !read_later(&Input::SourceGraphic) {
      self.source_graphic = None;
    }
    if !read_later(&Input::SourceAlpha) {
      self.source_alpha = None;
    }

    self.results.retain(|result| {
      index += 1;

      (keep_last && index - 1 == last)
        || later.iter().any(|primitive| {
          primitive
            .kind
            .inputs()
            .iter()
            .any(|input| input.names(&result.name))
        })
    });
  }

  fn apply_kind(&mut self, primitive: &Primitive, subregion: Rect<i32>) -> Option<Image> {
    let space = primitive.color_space;
    // The part of the result that the subregion keeps, which is all that pointwise primitives run on.
    let window = self.window(subregion);
    let pointwise = matches!(
      primitive.kind,
      Kind::ColorMatrix { .. } | Kind::ComponentTransfer { .. }
    );
    let mut inputs = primitive
      .kind
      .inputs()
      .into_iter()
      .map(|input| {
        let image = self.input(input)?;

        Some(if pointwise { image } else { image.resolved() })
      })
      .collect::<Option<Vec<_>>>()?
      .into_iter();

    self.release(false);

    let mut next = || inputs.next();

    match &primitive.kind {
      // `FEBlend` in `fe_blend.cc`.
      Kind::Blend { mode, .. } => {
        let top = next()?.into_color_space(space);
        let mut pixmap = next()?.into_color_space(space).take();

        draw(&mut pixmap, top.view(), 0, 0, Layer::Blend(*mode));
        Some(Image::new(pixmap, space))
      }
      // `FEDropShadow` in `fe_drop_shadow.cc`.
      Kind::DropShadow(shadow) => self.drop_shadow(shadow, next()?, space),
      // `FEFlood` in `fe_flood.cc`.
      Kind::Flood { color, opacity } => {
        let Color([red, green, blue, _]) = *color;
        let mut pixmap = self.blank()?;
        let width = pixmap.width() as usize;
        let flood = premultiply_pixel([red, green, blue, unorm_to_byte(*opacity)]);

        for row in
          pixmap.pixels_mut()[window.top * width..window.bottom * width].chunks_exact_mut(width)
        {
          row[window.left..window.right].fill(flood);
        }
        Some(Image::new(pixmap, ColorSpace::Srgb))
      }
      // `FEGaussianBlur` in `fe_gaussian_blur.cc`, blurred as CSS `blur()` is.
      Kind::GaussianBlur {
        std_dev_x,
        std_dev_y,
        ..
      } => {
        let input = next()?;
        let sigma = self.scaled(*std_dev_x, *std_dev_y);

        if sigma.x.max(sigma.y) <= MIN_SIGMA {
          return Some(input);
        }

        let mut pixmap = input.into_color_space(space).take();

        blur(&mut pixmap, sigma);
        Some(Image::new(pixmap, space))
      }
      // `FEOffset` in `fe_offset.cc`.
      Kind::Offset { dx, dy, .. } => {
        let input = next()?;
        let offset = self.scaled(*dx, *dy);

        if offset.x.approx_eq(0.0) && offset.y.approx_eq(0.0) {
          return Some(input);
        }

        let mut pixmap = self.blank()?;

        draw(
          &mut pixmap,
          input.view(),
          offset.x as i32,
          offset.y as i32,
          Layer::Over,
        );
        Some(Image::new(pixmap, input.color_space))
      }
      // `FEComposite` in `fe_composite.cc`.
      Kind::Composite { operator, .. } => {
        let top = next()?.into_color_space(space);
        let bottom = next()?.into_color_space(space);

        if let CompositeOperator::Arithmetic { k1, k2, k3, k4 } = *operator {
          let mut pixmap = self.blank()?;

          composite::arithmetic(
            [k1, k2, k3, k4],
            top.pixmap.pixels(),
            bottom.pixmap.pixels(),
            pixmap.pixels_mut(),
          );
          return Some(Image::new(pixmap, space));
        }

        let mut pixmap = bottom.take();
        let blend = match operator {
          CompositeOperator::In => Layer::In,
          CompositeOperator::Out => Layer::Out,
          CompositeOperator::Atop => Layer::Atop,
          CompositeOperator::Xor => Layer::Xor,
          CompositeOperator::Over | CompositeOperator::Arithmetic { .. } => Layer::Over,
        };

        draw(&mut pixmap, top.view(), 0, 0, blend);
        Some(Image::new(pixmap, space))
      }
      // `FEMerge` in `fe_merge.cc`.
      Kind::Merge { .. } => {
        let mut pixmap = self.blank()?;

        while let Some(input) = next() {
          draw(
            &mut pixmap,
            input.into_color_space(space).view(),
            0,
            0,
            Layer::Over,
          );
        }
        Some(Image::new(pixmap, space))
      }
      Kind::Tile { .. } => self.tile(next()?),
      // `SVGFEImage` in `core/svg/graphics/filters/svg_fe_image.cc`.
      Kind::Image(ops) => {
        let mut pixmap = self.blank()?;
        let (sx, sy) = self.transform.row_scales();

        draw_svg(
          ops,
          Affine {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            x: (subregion.left - self.region.left) as f32,
            y: (subregion.top - self.region.top) as f32,
          },
          &mut pixmap.as_mut(),
        );
        Some(Image::new(pixmap, ColorSpace::Srgb))
      }
      // `FEComponentTransfer` in `fe_component_transfer.cc`.
      Kind::ComponentTransfer { tables, .. } => {
        Some(self.color_filter(next()?, space, window, ColorFilter::Table(tables.clone())))
      }
      // `FEColorMatrix` in `fe_color_matrix.cc`, which CSS filter functions build as well.
      Kind::ColorMatrix { chain, .. } => {
        Some(self.color_filter(next()?, space, window, ColorFilter::Matrix(chain.clone())))
      }
      Kind::ConvolveMatrix(fe) => self.convolve_matrix(fe, next()?, space, window),
      Kind::Morphology {
        operator,
        radius_x,
        radius_y,
        ..
      } => self.morphology(*operator, (*radius_x, *radius_y), next()?, space),
      Kind::DisplacementMap(fe) => self.displacement_map(fe, next()?, next()?, space, window),
      Kind::Turbulence(fe) => self.turbulence(fe, primitive.subregion, window, space),
      Kind::DiffuseLighting(fe) => self.lighting(
        |light, color, depth| lighting::diffuse_lighting(fe, light, color, depth),
        fe.light_source,
        fe.lighting_color,
        next()?,
        space,
        subregion,
      ),
      Kind::SpecularLighting(fe) => self.lighting(
        |light, color, depth| lighting::specular_lighting(fe, light, color, depth),
        fe.light_source,
        fe.lighting_color,
        next()?,
        space,
        subregion,
      ),
    }
  }

  fn input(&self, input: &Input) -> Option<Image> {
    let image = match input {
      Input::SourceGraphic => self.source_graphic.clone(),
      Input::SourceAlpha => self.source_alpha.clone(),
      Input::Reference(name) => self
        .results
        .iter()
        .rev()
        .find(|result| result.name == *name)
        .map(|result| result.image.clone()),
    }?;

    debug_assert_eq!(
      (image.pixmap.width(), image.pixmap.height()),
      (self.region.width(), self.region.height()),
      "an input is as large as the filter region"
    );

    Some(image)
  }

  /// A pointwise primitive, deferred on `input` when its `subregion` covers the region.
  fn color_filter(
    &self,
    input: Image,
    space: ColorSpace,
    window: Rect<usize>,
    filter: ColorFilter,
  ) -> Image {
    let whole = self.window(self.region);
    let input = if input.filters.is_full() {
      input.resolved()
    } else {
      input
    };
    let image = input.in_color_space(space).with_filter(filter);

    if window == whole {
      image
    } else {
      image.resolved_within(window)
    }
  }

  fn window(&self, subregion: Rect<i32>) -> Rect<usize> {
    let (width, height) = (self.region.width() as i32, self.region.height() as i32);
    let column = |x: i32| (x - self.region.left).clamp(0, width) as usize;
    let row = |y: i32| (y - self.region.top).clamp(0, height) as usize;

    Rect {
      left: column(subregion.left),
      right: column(subregion.right),
      top: row(subregion.top),
      bottom: row(subregion.bottom),
    }
  }

  /// A transparent pixmap of the filter region.
  fn blank(&self) -> Option<Pixmap> {
    Pixmap::new(self.region.size())
  }

  /// The lengths along the axes of the filter's user space as lengths of the layer.
  fn scaled(&self, x: f32, y: f32) -> Point<f32> {
    let (sx, sy) = self.transform.row_scales();

    Point {
      x: x * sx,
      y: y * sy,
    }
  }

  fn drop_shadow(&self, shadow: &DropShadow, input: Image, space: ColorSpace) -> Option<Image> {
    let offset = self.scaled(shadow.dx, shadow.dy);
    let sigma = self.scaled(shadow.std_dev_x, shadow.std_dev_y);
    let input = input.into_color_space(space).take();
    let mut shadow_pixmap = input.clone();

    blur(&mut shadow_pixmap, sigma);

    let Color([red, green, blue, _]) = shadow.color;
    let opacity = unorm_to_byte(shadow.opacity);

    for pixel in shadow_pixmap.pixels_mut() {
      *pixel = premultiply_pixel([red, green, blue, mul_div_255(pixel[3], opacity)]);
    }

    // The shadow color is defined in sRGB, so only a linearRGB interpolation space needs a
    // conversion.
    let shadow_pixmap = Image::new(shadow_pixmap, ColorSpace::Srgb)
      .into_color_space(space)
      .take();

    let mut pixmap = self.blank()?;

    draw(
      &mut pixmap,
      shadow_pixmap.as_ref(),
      offset.x as i32,
      offset.y as i32,
      Layer::Over,
    );
    draw(&mut pixmap, input.as_ref(), 0, 0, Layer::Over);
    Some(Image::new(pixmap, space))
  }

  /// Blink's `FETile` in `fe_tile.cc`: the data of the input repeated across the region, copied as
  /// Skia's `SkTileImageFilter` copies it.
  fn tile(&self, input: Image) -> Option<Image> {
    let tile = input
      .region
      .translated(-self.region.left, -self.region.top)
      .intersection(&Rect::from_size(
        0,
        0,
        input.pixmap.width(),
        input.pixmap.height(),
      ))?;
    let (tile_width, tile_height) = (tile.width() as i32, tile.height() as i32);
    let mut pixmap = self.blank()?;
    let width = pixmap.width() as i32;
    let source_width = input.pixmap.width() as i32;
    let source = input.pixmap.pixels();
    let target = pixmap.pixels_mut();

    for (index, pixel) in target.iter_mut().enumerate() {
      let (x, y) = (index as i32 % width, index as i32 / width);
      let column = tile.left + (x - tile.left).rem_euclid(tile_width);
      let row = tile.top + (y - tile.top).rem_euclid(tile_height);

      *pixel = source[(row * source_width + column) as usize];
    }

    Some(Image::new(pixmap, input.color_space))
  }

  fn convolve_matrix(
    &self,
    fe: &ConvolveMatrix,
    input: Image,
    space: ColorSpace,
    window: Rect<usize>,
  ) -> Option<Image> {
    let input = input.into_color_space(space);
    let mut pixmap = input.blank_like()?;
    let width = pixmap.width() as usize;

    convolve_matrix::apply(
      fe,
      input.pixmap.pixels(),
      width,
      window,
      pixmap.pixels_mut(),
    );
    Some(Image::new(pixmap, space))
  }

  fn morphology(
    &self,
    operator: MorphologyOperator,
    radius: (f32, f32),
    input: Image,
    space: ColorSpace,
  ) -> Option<Image> {
    let mut pixmap = input.into_color_space(space).take();
    let radius = self.scaled(radius.0, radius.1);
    let width = pixmap.width() as usize;

    morphology::apply(operator, radius.x, radius.y, pixmap.pixels_mut(), width);
    Some(Image::new(pixmap, space))
  }

  fn displacement_map(
    &self,
    fe: &DisplacementMap,
    source: Image,
    map: Image,
    space: ColorSpace,
    window: Rect<usize>,
  ) -> Option<Image> {
    let source = source.into_color_space(space);
    let map = map.into_color_space(space);
    let mut pixmap = source.blank_like()?;
    let (sx, sy) = self.transform.row_scales();

    displacement_map::apply(
      fe,
      [sx, sy],
      pixmap.width() as usize,
      window,
      source.pixmap.pixels(),
      map.pixmap.pixels(),
      pixmap.pixels_mut(),
    );
    Some(Image::new(pixmap, space))
  }

  fn turbulence(
    &self,
    fe: &Turbulence,
    tile: Rect<f32>,
    window: Rect<usize>,
    space: ColorSpace,
  ) -> Option<Image> {
    let mut pixmap = self.blank()?;
    let (sx, sy) = self.transform.row_scales();

    if sx.approx_eq(0.0) || sy.approx_eq(0.0) {
      return Some(Image::new(pixmap, space));
    }

    let canvas = turbulence::Canvas {
      offset: (
        self.transform.x - self.region.left as f32,
        self.transform.y - self.region.top as f32,
      ),
      scale: (sx, sy),
    };
    let width = pixmap.width() as usize;

    turbulence::apply(
      fe,
      &canvas,
      (tile.width(), tile.height()),
      pixmap.pixels_mut(),
      width,
      window,
    );
    Some(Image::new(pixmap, space))
  }

  /// Lights `input`, clamping its samples to the edges Skia's `SkLightingImageFilter` picks: the
  /// window's edge where the input ends there, and one pixel past the window elsewhere.
  fn lighting(
    &self,
    lighting: impl FnOnce(LightSource, Color, f32) -> Lighting,
    light: LightSource,
    lighting_color: Color,
    input: Image,
    space: ColorSpace,
    subregion: Rect<i32>,
  ) -> Option<Image> {
    let mut pixmap = input.blank_like()?;
    let width = pixmap.width() as usize;
    let window = self.window(subregion);
    let local = |rect: Rect<i32>| Rect {
      left: (rect.left - self.region.left) as isize,
      right: (rect.right - self.region.left) as isize,
      top: (rect.top - self.region.top) as isize,
      bottom: (rect.bottom - self.region.top) as isize,
    };
    let (input_rect, output) = (local(input.region), local(subregion));
    let edge = |input: isize, output: isize, outset: isize| {
      if input == output { output } else { outset }
    };
    let edges = Rect {
      left: edge(input_rect.left, output.left, output.left - 1),
      right: edge(input_rect.right, output.right, output.right + 1),
      top: edge(input_rect.top, output.top, output.top - 1),
      bottom: edge(input_rect.bottom, output.bottom, output.bottom + 1),
    };
    let (light, depth) = self.light_source(light);

    lighting(light, convert_color(space, lighting_color), depth).apply(
      input.pixmap.pixels(),
      width,
      window,
      edges,
      pixmap.pixels_mut(),
    );
    Some(Image::new(pixmap, space))
  }

  /// The light source in the pixels of the filter region, and how far a unit of depth reaches
  /// there.
  fn light_source(&self, source: LightSource) -> (LightSource, f32) {
    let to_region = |x: f32, y: f32| {
      let (x, y) = self.transform.transform_point(x, y);

      (x - self.region.left as f32, y - self.region.top as f32)
    };
    let (scale_x, scale_y) = self.transform.row_scales();
    let depth = scale_x.hypot(scale_y) / SQRT_2;

    let light = match source {
      LightSource::Distant { .. } => source,
      LightSource::Point { x, y, z } => {
        let (x, y) = to_region(x, y);

        LightSource::Point { x, y, z: z * depth }
      }
      LightSource::Spot(light) => {
        let (x, y) = to_region(light.x, light.y);
        let (points_at_x, points_at_y) = to_region(light.points_at_x, light.points_at_y);

        LightSource::Spot(SpotLight {
          x,
          y,
          z: light.z * depth,
          points_at_x,
          points_at_y,
          points_at_z: light.points_at_z * depth,
          ..light
        })
      }
    };

    (light, depth)
  }
}

/// How [`draw`] joins a source to the target below it: source-over, the Porter-Duff operators of
/// `feComposite`, or a blend mode.
#[derive(Clone, Copy)]
enum Layer {
  Over,
  In,
  Out,
  Atop,
  Xor,
  Blend(BlendMode),
}

/// Draws `source` onto `target` with its top-left corner at `x`, `y`, joined as `layer` has it.
/// The Porter-Duff operators follow Skia's 8-bit `srcin`, `srcout`, `srcatop`, and `xor` stages.
fn draw(target: &mut Pixmap, source: PixmapRef, x: i32, y: i32, layer: Layer) {
  if let Layer::Over = layer {
    draw_over(&mut target.as_mut(), source, (x, y), 1.0, None);
    return;
  }

  let target_width = target.width() as i32;
  let (source_width, source_height) = (source.width() as i32, source.height() as i32);
  let left = x.max(0);
  let right = (x + source_width).min(target_width);
  let top = y.max(0);
  let bottom = (y + source_height).min(target.height() as i32);
  let target_pixels = target.pixels_mut();
  let source_pixels = source.pixels();

  for row in top..bottom {
    for column in left..right {
      let destination = &mut target_pixels[(row * target_width + column) as usize];
      let source = source_pixels[((row - y) * source_width + column - x) as usize];
      let (source_alpha, destination_alpha) = (source[3], destination[3]);

      *destination = match layer {
        Layer::Over => {
          let mut over = *destination;

          composite_premultiplied_over(&mut over, source);
          over
        }
        Layer::In => scale_premultiplied_pixel(source, destination_alpha),
        Layer::Out => scale_premultiplied_pixel(source, 255 - destination_alpha),
        Layer::Atop => std::array::from_fn(|lane| {
          div_255(
            u16::from(source[lane]) * u16::from(destination_alpha)
              + u16::from(destination[lane]) * u16::from(255 - source_alpha),
          )
        }),
        Layer::Xor => std::array::from_fn(|lane| {
          div_255(
            u16::from(source[lane]) * u16::from(255 - destination_alpha)
              + u16::from(destination[lane]) * u16::from(255 - source_alpha),
          )
        }),
        Layer::Blend(mode) => {
          let mut blended = *destination;

          blend_premultiplied_pixel(&mut blended, source, mode);
          blended
        }
      };
    }
  }
}

/// Blurs premultiplied pixels by a sigma for each axis.
fn blur(pixmap: &mut Pixmap, sigma: Point<f32>) {
  let (width, height) = (pixmap.width() as usize, pixmap.height() as usize);

  blur_rgba_axes(pixmap.pixels_mut(), width, height, sigma);
}

/// Clears the pixels outside `keep`.
fn clear_outside(pixmap: &mut Pixmap, keep: Rect<i32>) {
  let (width, height) = (pixmap.width() as usize, pixmap.height() as usize);
  let column = |x: i32| x.clamp(0, width as i32) as usize;
  let row = |y: i32| y.clamp(0, height as i32) as usize;
  let (left, right) = (column(keep.left), column(keep.right));
  let (top, bottom) = (row(keep.top), row(keep.bottom));
  let pixels = pixmap.pixels_mut();

  pixels[..top * width].fill([0; 4]);
  pixels[bottom * width..].fill([0; 4]);
  for line in pixels[top * width..bottom * width].chunks_exact_mut(width) {
    line[..left].fill([0; 4]);
    line[right..].fill([0; 4]);
  }
}

/// The pixels `bounds` covers, none unless it has area, rounded out as Skia's crop image filter
/// rounds a decal crop rect (`SkCropImageFilter.cpp`). Edges past `i32` saturate, so a huge region
/// still meets the layer it covers.
fn pixel_bounds(bounds: Rect<f32>) -> Option<Rect<i32>> {
  (bounds.left < bounds.right && bounds.top < bounds.bottom).then(|| Rect {
    left: bounds.left.floor() as i32,
    top: bounds.top.floor() as i32,
    right: bounds.right.ceil() as i32,
    bottom: bounds.bottom.ceil() as i32,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_region_past_the_i32_range_keeps_the_pixels_it_covers() {
    let bounds = pixel_bounds(Rect::from_ltrb(-100.0, 0.0, 3e9, 16.0).unwrap()).unwrap();

    assert_eq!(
      bounds.intersection(&Rect::from_size(0, 0, 16, 16)),
      Some(Rect::from_size(0, 0, 16, 16))
    );
  }
}
