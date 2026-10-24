// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Running the primitives of a filter on a layer. The structure is resvg's `filter/mod.rs`, with
//! Blink's `FilterEffect` in `filter_effect.cc` as the model for what each primitive reads.

use std::{f32::consts::SQRT_2, rc::Rc};

use tiny_skia::{BlendMode as TinyBlendMode, IntRect, Pixmap, PixmapPaint, PixmapRef, Transform};

use super::{
  ColorSpace, CompositeOperator, ConvolveMatrix, DiffuseLighting, DisplacementMap, DropShadow,
  Input, Kind, LightSource, MorphologyOperator, Primitive, SpecularLighting, SpotLight, SvgFilter,
  Turbulence, composite, convolve_matrix, displacement_map, lighting, morphology, turbulence,
};
use crate::{
  blur::{MIN_SIGMA, blur_rgba_axes},
  geometry::{Point, Rect},
  resources::{
    image::{MAX_RASTER_PIXELS, within_raster_pixel_budget},
    image_buffer::{demultiply_pixel, demultiply_rgba_in_place, premultiply_pixel},
  },
  style::{Affine, Color, TransferTable, math::mul_div_255},
  svg::approx_eq::ApproxEq,
  svg_drawing::{SvgDrawing, tiny_blend_mode},
};

/// A primitive result: the pixels of the filter region, and where they hold data.
#[derive(Clone)]
struct Image {
  pixmap: Rc<Pixmap>,
  /// The part of the filter region with data, in the region's pixels. The rest is transparent.
  region: Rect<i32>,
  color_space: ColorSpace,
}

impl Image {
  fn new(pixmap: Pixmap, color_space: ColorSpace) -> Self {
    Self {
      region: Rect::from_size(0, 0, pixmap.width(), pixmap.height()),
      pixmap: Rc::new(pixmap),
      color_space,
    }
  }

  /// The pixels in `color_space`.
  fn into_color_space(self, color_space: ColorSpace) -> Self {
    if color_space == self.color_space {
      return self;
    }

    let region = self.region;
    let from = self.color_space;
    let mut pixmap = self.take();

    from.convert(color_space, pixmap.data_mut());

    Self {
      pixmap: Rc::new(pixmap),
      region,
      color_space,
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

  /// The pixels, copied only when another result still reads them.
  fn take(self) -> Pixmap {
    Rc::unwrap_or_clone(self.pixmap)
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
      image.region = subregion;
    }

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
    let mut inputs = primitive
      .kind
      .inputs()
      .into_iter()
      .map(|input| self.input(input))
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
            subregion.left as f32,
            subregion.top as f32,
          ),
          &mut pixmap.as_mut(),
        );
        Some(Image::new(pixmap, ColorSpace::Srgb))
      }
      // `FEComponentTransfer` in `fe_component_transfer.cc`.
      Kind::ComponentTransfer { tables, .. } => {
        let mut pixmap = next()?.into_color_space(space).take();

        map_straight(&mut pixmap, window, |pixel| transfer(tables, pixel));
        Some(Image::new(pixmap, space))
      }
      // `FEColorMatrix` in `fe_color_matrix.cc`, which CSS filter functions build as well.
      Kind::ColorMatrix { chain, .. } => {
        let mut pixmap = next()?.into_color_space(space).take();

        map_straight(&mut pixmap, window, |pixel| chain.apply_rgba8(pixel));
        Some(Image::new(pixmap, space))
      }
      Kind::ConvolveMatrix(fe) => self.convolve_matrix(fe, next()?, space),
      Kind::Morphology {
        operator,
        radius_x,
        radius_y,
        ..
      } => self.morphology(*operator, (*radius_x, *radius_y), next()?, space),
      Kind::DisplacementMap(fe) => self.displacement_map(fe, next()?, next()?, space),
      Kind::Turbulence(fe) => self.turbulence(fe, window, space),
      Kind::DiffuseLighting(fe) => self.diffuse_lighting(fe, next()?, space),
      Kind::SpecularLighting(fe) => self.specular_lighting(fe, next()?, space),
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
    ColorSpace::Srgb.convert(space, shadow_pixmap.data_mut());

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
    Some(Image::new(pixmap, ColorSpace::Srgb))
  }

  fn convolve_matrix(&self, fe: &ConvolveMatrix, input: Image, space: ColorSpace) -> Option<Image> {
    let mut pixmap = input.into_color_space(space).take();

    if fe.preserve_alpha {
      demultiply_rgba_in_place(pixmap.data_mut());
    }

    let width = pixmap.width() as usize;

    convolve_matrix::apply(fe, pixmap.rgba_mut(), width);
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

    // A radius of zero on either axis clears the result.
    if radius.x > 0.0 && radius.y > 0.0 {
      let width = pixmap.width() as usize;

      morphology::apply(operator, radius.x, radius.y, pixmap.rgba_mut(), width);
    } else {
      pixmap.fill(tiny_skia::Color::TRANSPARENT);
    }

    Some(Image::new(pixmap, space))
  }

  fn displacement_map(
    &self,
    fe: &DisplacementMap,
    source: Image,
    map: Image,
    space: ColorSpace,
  ) -> Option<Image> {
    let source = source.into_color_space(space);
    let mut map = map.into_color_space(space).take();
    let mut pixmap = source.blank_like()?;
    let (sx, sy) = self.transform.row_scales();

    demultiply_rgba_in_place(map.data_mut());
    displacement_map::apply(
      fe,
      [sx, sy],
      pixmap.width() as usize,
      source.pixmap.rgba(),
      map.rgba(),
      pixmap.rgba_mut(),
    );
    Some(Image::new(pixmap, space))
  }

  fn turbulence(&self, fe: &Turbulence, window: Rect<usize>, space: ColorSpace) -> Option<Image> {
    let mut pixmap = self.blank()?;
    let (sx, sy) = self.transform.row_scales();

    if sx.approx_eq(0.0) || sy.approx_eq(0.0) {
      return Some(Image::new(pixmap, space));
    }

    let canvas = turbulence::Canvas {
      offset: (
        f64::from(self.region.left) - f64::from(self.transform.x),
        f64::from(self.region.top) - f64::from(self.transform.y),
      ),
      scale: (f64::from(sx), f64::from(sy)),
    };
    let width = pixmap.width() as usize;

    turbulence::apply(fe, &canvas, pixmap.rgba_mut(), width, window);
    for row in pixmap.rgba_mut()[window.top * width..window.bottom * width].chunks_exact_mut(width)
    {
      for pixel in &mut row[window.left..window.right] {
        *pixel = premultiply_pixel(*pixel);
      }
    }

    Some(Image::new(pixmap, space))
  }

  fn diffuse_lighting(
    &self,
    fe: &DiffuseLighting,
    input: Image,
    space: ColorSpace,
  ) -> Option<Image> {
    let mut pixmap = input.blank_like()?;
    let light_source = self.light_source(fe.light_source);
    let width = pixmap.width() as usize;

    lighting::diffuse_lighting(
      fe,
      light_source,
      input.pixmap.rgba(),
      width,
      pixmap.rgba_mut(),
    );
    Some(Image::new(pixmap, space))
  }

  fn specular_lighting(
    &self,
    fe: &SpecularLighting,
    input: Image,
    space: ColorSpace,
  ) -> Option<Image> {
    let mut pixmap = input.blank_like()?;
    let light_source = self.light_source(fe.light_source);
    let width = pixmap.width() as usize;

    lighting::specular_lighting(
      fe,
      light_source,
      input.pixmap.rgba(),
      width,
      pixmap.rgba_mut(),
    );
    Some(Image::new(pixmap, space))
  }

  /// The light source in the pixels of the filter region.
  fn light_source(&self, source: LightSource) -> LightSource {
    let to_region = |x: f32, y: f32| {
      let (x, y) = self.transform.transform_point(x, y);

      (x - self.region.left as f32, y - self.region.top as f32)
    };
    let depth = self.transform.a.hypot(self.transform.d) / SQRT_2;

    match source {
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
    }
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

/// Runs `map` on the straight-alpha pixels, which Filter Effects defines the color primitives on.
fn map_straight(pixmap: &mut Pixmap, window: Rect<usize>, map: impl Fn([u8; 4]) -> [u8; 4]) {
  let width = pixmap.width() as usize;
  let rows = &mut pixmap.rgba_mut()[window.top * width..window.bottom * width];

  for row in rows.chunks_exact_mut(width) {
    for pixel in &mut row[window.left..window.right] {
      demultiply_pixel(pixel);
      *pixel = premultiply_pixel(map(*pixel));
    }
  }
}

/// A pixel with each channel through its table, if any.
fn transfer(tables: &[Option<TransferTable>; 4], mut pixel: [u8; 4]) -> [u8; 4] {
  for (channel, table) in pixel.iter_mut().zip(tables) {
    if let Some(table) = table {
      *channel = table[usize::from(*channel)];
    }
  }
  pixel
}
