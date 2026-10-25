// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Running the primitives of a filter on a layer. The structure is resvg's `filter/mod.rs`, with
//! Blink's `FilterEffect` in `filter_effect.cc` as the model for what each primitive reads.

use std::{f32::consts::SQRT_2, rc::Rc};

use tiny_skia::{BlendMode as TinyBlendMode, IntRect, Pixmap, PixmapPaint, PixmapRef, Transform};

use super::{
  ColorSpace, CompositeOperator, ConvolveMatrix, DisplacementMap, DropShadow, Input, Kind,
  LightSource, MorphologyOperator, Primitive, SpotLight, SvgFilter, Turbulence,
  color_filter::{ColorFilter, ColorFilters},
  composite,
  lighting::Lighting,
  turbulence,
};
use crate::{
  blur::{MIN_SIGMA, blur_rgba_axes},
  geometry::{Point, Rect},
  resources::{
    image::{MAX_RASTER_PIXELS, within_raster_pixel_budget},
    image_buffer::premultiply_pixel,
  },
  style::{Affine, Color, math::mul_div_255},
  svg::approx_eq::ApproxEq,
  svg_drawing::{SvgDrawing, draw_over, tiny_blend_mode},
};

/// A primitive result: the pixels of the filter region, and where they hold data.
///
/// The pixels hold the colors before `filters`, which Skia's `FilterResult` likewise defers so a
/// run of color filters stores bytes once. Every reader but a pointwise primitive resolves them.
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

    for line in pixmap.rgba_mut()[window.top * width..window.bottom * width].chunks_exact_mut(width)
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
    Pixmap::new(self.pixmap.width(), self.pixmap.height())
  }

  /// The image with its colors black, as `SourceAlpha` is.
  fn alpha(&self) -> Self {
    let mut pixmap = (*self.pixmap).clone();

    for pixel in pixmap.rgba_mut() {
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

impl SvgFilter {
  /// Applies the filter to a layer, which `transform` maps the user space of the filter onto. A
  /// filter that cannot run clears the layer.
  pub(crate) fn apply(&self, transform: Transform, layer: &mut Pixmap) {
    let (width, height) = (layer.width(), layer.height());
    let Some(blank) = Pixmap::new(width, height) else {
      return;
    };
    let source = std::mem::replace(layer, blank);

    if let Some((image, region)) = self.run(transform.into(), source) {
      if region == Rect::from_size(0, 0, width, height) {
        *layer = image.take();
      } else {
        draw(
          layer,
          image.view(),
          region.left,
          region.top,
          TinyBlendMode::SourceOver,
        );
      }
    }
  }

  /// The output of the primitives on the layer, in sRGB, and the part of the layer it covers.
  fn run(&self, transform: Affine, source: Pixmap) -> Option<(Image, Rect<i32>)> {
    // The filter region comes straight from the filter, so it can be larger than the layer.
    let layer = Rect::from_size(0, 0, source.width(), source.height());
    let region = self.region.pixel_bounds(transform)?.intersection(&layer)?;
    // Every input of a primitive is as large as the region.
    let source = if region == layer {
      source
    } else {
      source.clone_rect(IntRect::from_xywh(
        region.left,
        region.top,
        region.width(),
        region.height(),
      )?)?
    };
    let mut graphic = Image::new(source, ColorSpace::Srgb);

    graphic.region = region;

    let alpha = self
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
      later: &self.primitives,
    };

    for (index, primitive) in self.primitives.iter().enumerate() {
      run.later = &self.primitives[index + 1..];

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

  /// Applies the filter to a premultiplied RGBA layer in place. It runs on a pixmap that covers
  /// the part of the region the layer overlaps, with the layer placed at the region offset and
  /// the window copied back afterwards. None when the pixmap is empty or past the raster budget.
  pub(crate) fn apply_to_layer(&self, layer: &mut [u8], width: u32, height: u32) -> Option<()> {
    // The pixels of the region with its top-left corner at the origin, which the layer sits within
    // at `dx`, `dy`.
    let dx = (-self.region.left).round() as i32;
    let dy = (-self.region.top).round() as i32;
    let window = self
      .region
      .pixel_bounds(Transform::from_translate(-self.region.left, -self.region.top).into())?
      .intersection(&Rect::from_size(dx, dy, width, height))?;

    if !within_raster_pixel_budget(window.width(), window.height()) {
      return None;
    }

    let mut sub = Pixmap::new(window.width(), window.height())?;
    let transform = Transform::from_translate(
      -self.region.left - window.left as f32,
      -self.region.top - window.top as f32,
    );

    draw(
      &mut sub,
      PixmapRef::from_bytes(layer, width, height)?,
      dx - window.left,
      dy - window.top,
      TinyBlendMode::SourceOver,
    );
    self.apply(transform, &mut sub);

    let mut out = Pixmap::new(width, height)?;

    draw(
      &mut out,
      sub.as_ref(),
      window.left - dx,
      window.top - dy,
      TinyBlendMode::SourceOver,
    );
    layer.copy_from_slice(out.data());

    Some(())
  }
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
    let mut subregion = primitive.subregion.pixel_bounds(self.transform)?;

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

  /// Drops the results that no later primitive reads, so the pixels of a result that its last
  /// reader holds alone are taken without a copy. `keep_last` keeps the newest result, which is
  /// the output when nothing reads it.
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

        draw(&mut pixmap, top.view(), 0, 0, tiny_blend_mode(*mode));
        Some(Image::new(pixmap, space))
      }
      // `FEDropShadow` in `fe_drop_shadow.cc`.
      Kind::DropShadow(shadow) => self.drop_shadow(shadow, next()?, space),
      // `FEFlood` in `fe_flood.cc`.
      Kind::Flood { color, opacity } => {
        let Color([red, green, blue, _]) = *color;
        let mut pixmap = self.blank()?;
        let width = pixmap.width() as usize;
        let flood = premultiply_pixel([red, green, blue, (opacity * 255.0 + 0.5) as u8]);

        for row in
          pixmap.rgba_mut()[window.top * width..window.bottom * width].chunks_exact_mut(width)
        {
          row[window.left..window.right].fill(flood);
        }
        Some(Image::new(pixmap, ColorSpace::Srgb))
      }
      // `FEGaussianBlur` in `fe_gaussian_blur.cc`, which Skia blurs as CSS `blur()` is.
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
          TinyBlendMode::SourceOver,
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
            top.pixmap.rgba(),
            bottom.pixmap.rgba(),
            pixmap.rgba_mut(),
          );
          return Some(Image::new(pixmap, space));
        }

        let mut pixmap = bottom.take();
        let blend = match operator {
          CompositeOperator::In => TinyBlendMode::SourceIn,
          CompositeOperator::Out => TinyBlendMode::SourceOut,
          CompositeOperator::Atop => TinyBlendMode::SourceAtop,
          CompositeOperator::Xor => TinyBlendMode::Xor,
          CompositeOperator::Over | CompositeOperator::Arithmetic { .. } => {
            TinyBlendMode::SourceOver
          }
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
            TinyBlendMode::SourceOver,
          );
        }
        Some(Image::new(pixmap, space))
      }
      Kind::Tile { .. } => self.tile(next()?),
      // `SVGFEImage` in `core/svg/graphics/filters/svg_fe_image.cc`.
      Kind::Image(ops) => {
        let mut pixmap = self.blank()?;
        let (sx, sy) = self.transform.row_scales();

        SvgDrawing.draw(
          ops,
          Transform::from_row(
            sx,
            0.0,
            0.0,
            sy,
            (subregion.left - self.region.left) as f32,
            (subregion.top - self.region.top) as f32,
          ),
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
        |light, color, depth| fe.lighting(light, color, depth),
        fe.light_source,
        fe.lighting_color,
        next()?,
        space,
        subregion,
      ),
      Kind::SpecularLighting(fe) => self.lighting(
        |light, color, depth| fe.lighting(light, color, depth),
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

  /// `subregion` in the pixels of the filter region, cut to it.
  /// A pointwise primitive: `filter` deferred on `input` when it covers the whole region, and run
  /// over its window otherwise, as the subregion clears the rest.
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
    Pixmap::new(self.region.width(), self.region.height())
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
    let opacity = (shadow.opacity * 255.0 + 0.5) as u8;

    for pixel in shadow_pixmap.rgba_mut() {
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
      TinyBlendMode::SourceOver,
    );
    draw(&mut pixmap, input.as_ref(), 0, 0, TinyBlendMode::SourceOver);
    Some(Image::new(pixmap, space))
  }

  /// Blink's `FETile` in `fe_tile.cc`: the data of the input repeated across the region.
  fn tile(&self, input: Image) -> Option<Image> {
    let tile = input.region.translated(-self.region.left, -self.region.top);
    let tile_pixmap = input.pixmap.clone_rect(tiny_skia::IntRect::from_xywh(
      tile.left,
      tile.top,
      tile.width(),
      tile.height(),
    )?)?;
    let mut pixmap = self.blank()?;
    let rect = tiny_skia::Rect::from_xywh(
      0.0,
      0.0,
      self.region.width() as f32,
      self.region.height() as f32,
    )?;

    pixmap.fill_rect(
      rect,
      &tiny_skia::Paint {
        shader: tiny_skia::Pattern::new(
          tile_pixmap.as_ref(),
          tiny_skia::SpreadMode::Repeat,
          tiny_skia::FilterQuality::Bicubic,
          1.0,
          Transform::from_translate(tile.left as f32, tile.top as f32),
        ),
        ..tiny_skia::Paint::default()
      },
      Transform::identity(),
      None,
    );
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

    fe.apply(input.pixmap.rgba(), width, window, pixmap.rgba_mut());
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

    operator.apply(radius.x, radius.y, pixmap.rgba_mut(), width);
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

    fe.apply(
      [sx, sy],
      pixmap.width() as usize,
      window,
      source.pixmap.rgba(),
      map.pixmap.rgba(),
      pixmap.rgba_mut(),
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

    fe.apply(
      &canvas,
      (tile.width(), tile.height()),
      pixmap.rgba_mut(),
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

    lighting(light, space.convert_color(lighting_color), depth).apply(
      input.pixmap.rgba(),
      width,
      window,
      edges,
      pixmap.rgba_mut(),
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

impl Kind {
  /// The inputs the primitive reads, in the order it takes them.
  fn inputs(&self) -> Vec<&Input> {
    match self {
      Kind::Blend { input1, input2, .. } | Kind::Composite { input1, input2, .. } => {
        vec![input1, input2]
      }
      Kind::DisplacementMap(fe) => vec![&fe.input1, &fe.input2],
      Kind::Merge { inputs } => inputs.iter().collect(),
      Kind::ColorMatrix { input, .. }
      | Kind::ComponentTransfer { input, .. }
      | Kind::GaussianBlur { input, .. }
      | Kind::Morphology { input, .. }
      | Kind::Offset { input, .. }
      | Kind::Tile { input } => vec![input],
      Kind::ConvolveMatrix(fe) => vec![&fe.input],
      Kind::DiffuseLighting(fe) => vec![&fe.input],
      Kind::SpecularLighting(fe) => vec![&fe.input],
      Kind::DropShadow(fe) => vec![&fe.input],
      Kind::Flood { .. } | Kind::Image(_) | Kind::Turbulence(_) => Vec::new(),
    }
  }
}

/// Pixmap bytes as RGBA pixels.
trait PixmapPixels {
  fn rgba(&self) -> &[[u8; 4]];
  fn rgba_mut(&mut self) -> &mut [[u8; 4]];
}

impl PixmapPixels for Pixmap {
  fn rgba(&self) -> &[[u8; 4]] {
    self.data().as_chunks().0
  }

  fn rgba_mut(&mut self) -> &mut [[u8; 4]] {
    self.data_mut().as_chunks_mut().0
  }
}

/// Draws `source` over `target` with its top-left corner at `x`, `y`.
fn draw(target: &mut Pixmap, source: PixmapRef, x: i32, y: i32, blend_mode: TinyBlendMode) {
  if blend_mode == TinyBlendMode::SourceOver {
    draw_over(&mut target.as_mut(), source, (x, y), 1.0, None);
    return;
  }

  target.draw_pixmap(
    x,
    y,
    source,
    &PixmapPaint {
      blend_mode,
      ..PixmapPaint::default()
    },
    Transform::identity(),
    None,
  );
}

/// Blurs premultiplied pixels by a sigma for each axis.
fn blur(pixmap: &mut Pixmap, sigma: Point<f32>) {
  let (width, height) = (pixmap.width() as usize, pixmap.height() as usize);

  blur_rgba_axes(pixmap.rgba_mut(), width, height, sigma);
}

/// Clears the pixels outside `keep`.
fn clear_outside(pixmap: &mut Pixmap, keep: Rect<i32>) {
  let (width, height) = (pixmap.width() as usize, pixmap.height() as usize);
  let column = |x: i32| x.clamp(0, width as i32) as usize;
  let row = |y: i32| y.clamp(0, height as i32) as usize;
  let (left, right) = (column(keep.left), column(keep.right));
  let (top, bottom) = (row(keep.top), row(keep.bottom));
  let pixels = pixmap.rgba_mut();

  pixels[..top * width].fill([0; 4]);
  pixels[bottom * width..].fill([0; 4]);
  for line in pixels[top * width..bottom * width].chunks_exact_mut(width) {
    line[..left].fill([0; 4]);
    line[right..].fill([0; 4]);
  }
}
