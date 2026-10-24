//! An SVG image written as the vector ops takumi-core flattens it to, so its text draws in the
//! fonts the render registered instead of whatever the viewer falls back to.

use std::{borrow::Cow, io};

use takumi_core::{
  path_data::path_data,
  resources::image::{
    SvgGradient, SvgImageData, SvgLineCap, SvgLineJoin, SvgOp, SvgPaint, SvgSpreadMethod,
    SvgStrokeStyle, sniff_mime, to_data_url,
  },
  style::{Affine, FillRule, ImageScalingAlgorithm, ToCss},
};

use crate::{Frame, GradientStop, Rgba, SvgDocument, matrix_attr, num, push_opacity};

/// Whether SVG elements can express all of `ops`, which leaves out filters.
pub(crate) fn is_vector(ops: &[SvgOp]) -> bool {
  ops.iter().all(|op| match op {
    SvgOp::PushFilter(_) => false,
    SvgOp::PushMask { ops, .. } => is_vector(ops),
    SvgOp::Draw { fill, stroke, .. } => fill
      .iter()
      .map(|fill| &fill.paint)
      .chain(stroke.iter().map(|stroke| &stroke.paint))
      .all(|paint| match paint {
        SvgPaint::Pattern { ops, .. } => is_vector(ops),
        _ => true,
      }),
    _ => true,
  })
}

impl SvgDocument {
  /// Writes `ops`, each `Push*` as a `<g>` its `Pop` closes. `is_vector` keeps filters out.
  pub(crate) fn svg_ops(&mut self, ops: &[SvgOp]) -> io::Result<()> {
    let mut groups = Vec::new();

    for op in ops {
      match op {
        SvgOp::PushTransform(transform) => {
          groups.push(self.begin_group(affine(*transform), 1.0, None, None)?);
        }
        SvgOp::PushClip { path, evenodd } => {
          let clip = self.clip_path(
            &path_data(path, Affine::IDENTITY),
            fill_rule(*evenodd),
            None,
          )?;

          groups.push(self.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)?);
        }
        SvgOp::PushBlend(blend) => groups.push(self.begin_blend_group(&blend.to_css_string())?),
        SvgOp::PushOpacity(opacity) => {
          groups.push(self.begin_group(Affine::IDENTITY, *opacity, None, None)?);
        }
        SvgOp::PushMask { ops, luminance } => {
          let (token, reference) = self.begin_mask(*luminance)?;

          self.svg_ops(ops)?;
          self.end_mask(token)?;
          groups.push(self.begin_masked_group(&reference)?);
        }
        SvgOp::Pop => {
          if let Some(group) = groups.pop() {
            self.end_group(group)?;
          }
        }
        SvgOp::Draw { path, fill, stroke } => {
          let mut attrs: Vec<(&str, Cow<'_, str>)> =
            vec![("d", path_data(path, Affine::IDENTITY).into())];

          match fill {
            Some(fill) => {
              attrs.push(("fill", self.svg_paint(&fill.paint)?.into()));
              push_opacity(&mut attrs, "fill-opacity", fill.opacity);
              if fill.evenodd {
                attrs.push(("fill-rule", "evenodd".into()));
              }
            }
            None => attrs.push(("fill", "none".into())),
          }
          if let Some(stroke) = stroke {
            self.push_stroke(&mut attrs, stroke)?;
          }
          self.empty("path", &attrs)?;
        }
        // `is_vector` keeps filters out; the matching `Pop` still needs a group.
        SvgOp::PushFilter(_) => {
          groups.push(self.begin_group(Affine::IDENTITY, 1.0, None, None)?);
        }
        SvgOp::Image {
          data,
          width,
          height,
          rendering,
        } => {
          let Some(href) = image_href(data) else {
            continue;
          };
          let mut attrs = Frame::new(0.0, 0.0, *width, *height).attrs();

          attrs.push(("href", href.into()));
          if *rendering == ImageScalingAlgorithm::Pixelated {
            attrs.push(("image-rendering", "pixelated".into()));
          }
          self.empty("image", &attrs)?;
        }
      }
    }

    Ok(())
  }

  fn push_stroke<'a>(
    &mut self,
    attrs: &mut Vec<(&'a str, Cow<'a, str>)>,
    stroke: &'a SvgStrokeStyle,
  ) -> io::Result<()> {
    attrs.push(("stroke", self.svg_paint(&stroke.paint)?.into()));
    push_opacity(attrs, "stroke-opacity", stroke.opacity);
    attrs.push(("stroke-width", num(stroke.width).into()));
    if stroke.miter_limit != 4.0 {
      attrs.push(("stroke-miterlimit", num(stroke.miter_limit).into()));
    }
    match stroke.cap {
      SvgLineCap::Butt => {}
      SvgLineCap::Round => attrs.push(("stroke-linecap", "round".into())),
      SvgLineCap::Square => attrs.push(("stroke-linecap", "square".into())),
    }
    match stroke.join {
      SvgLineJoin::Miter => {}
      SvgLineJoin::Round => attrs.push(("stroke-linejoin", "round".into())),
      SvgLineJoin::Bevel => attrs.push(("stroke-linejoin", "bevel".into())),
    }
    if let Some((array, offset)) = &stroke.dash {
      let array: Vec<String> = array.iter().copied().map(num).collect();

      attrs.push(("stroke-dasharray", array.join(" ").into()));
      attrs.push(("stroke-dashoffset", num(*offset).into()));
    }

    Ok(())
  }

  /// The `fill` or `stroke` value for `paint`, defining its gradient or pattern first.
  fn svg_paint(&mut self, paint: &SvgPaint) -> io::Result<String> {
    match paint {
      SvgPaint::Color([red, green, blue]) => Ok(Rgba([*red, *green, *blue, 255]).hex()),
      SvgPaint::Linear {
        start,
        end,
        gradient,
      } => self.svg_gradient(
        "linearGradient",
        "lg",
        vec![
          ("x1", num(start.x).into()),
          ("y1", num(start.y).into()),
          ("x2", num(end.x).into()),
          ("y2", num(end.y).into()),
        ],
        gradient,
      ),
      SvgPaint::Radial {
        center,
        radius,
        focal,
        gradient,
      } => self.svg_gradient(
        "radialGradient",
        "rg",
        vec![
          ("cx", num(center.x).into()),
          ("cy", num(center.y).into()),
          ("r", num(*radius).into()),
          ("fx", num(focal.x).into()),
          ("fy", num(focal.y).into()),
        ],
        gradient,
      ),
      SvgPaint::Pattern {
        ops,
        transform,
        width,
        height,
      } => {
        let (id, reference) = self.alloc_id("pat");

        self.open(
          "pattern",
          &[
            ("id", id.into()),
            ("patternUnits", "userSpaceOnUse".into()),
            ("patternTransform", matrix(*transform).into()),
            ("width", num(*width).into()),
            ("height", num(*height).into()),
          ],
        )?;
        self.svg_ops(ops)?;
        self.close("pattern")?;

        Ok(reference)
      }
    }
  }

  /// Defines `gradient` as a `name` element with `geometry`, and returns its `url(#id)`.
  fn svg_gradient(
    &mut self,
    name: &str,
    id_prefix: &str,
    mut geometry: Vec<(&str, Cow<'_, str>)>,
    gradient: &SvgGradient,
  ) -> io::Result<String> {
    let transform = affine(gradient.transform);
    let stops: Vec<GradientStop> = gradient
      .stops
      .iter()
      .map(|stop| {
        let [red, green, blue] = stop.color;

        GradientStop {
          offset: stop.offset,
          color: Rgba([red, green, blue, (stop.opacity * 255.0).round() as u8]),
        }
      })
      .collect();

    if !transform.is_identity() {
      geometry.push(("gradientTransform", matrix_attr(transform).into()));
    }
    if gradient.spread == SvgSpreadMethod::Reflect {
      geometry.push(("spreadMethod", "reflect".into()));
    }

    self.gradient(
      name,
      id_prefix,
      gradient.spread == SvgSpreadMethod::Repeat,
      geometry,
      &stops,
    )
  }
}

/// The `data:` URL of an image op's pixels.
fn image_href(data: &SvgImageData) -> Option<String> {
  match data {
    SvgImageData::Encoded(bytes) => Some(to_data_url(sniff_mime(bytes), bytes)),
    SvgImageData::Bitmap(buffer) => buffer
      .encode_png()
      .map(|png| to_data_url("image/png", &png)),
  }
}

/// `matrix(...)` for an op's `(sx, ky, kx, sy, tx, ty)` transform.
fn matrix(transform: [f32; 6]) -> String {
  matrix_attr(affine(transform))
}

fn affine([a, b, c, d, x, y]: [f32; 6]) -> Affine {
  Affine { a, b, c, d, x, y }
}

fn fill_rule(evenodd: bool) -> FillRule {
  if evenodd {
    FillRule::EvenOdd
  } else {
    FillRule::NonZero
  }
}
