//! An SVG image written as the vector ops takumi-core flattens it to, so its text draws in the
//! fonts the render registered instead of whatever the viewer falls back to.

use std::{
  borrow::Cow,
  hash::{DefaultHasher, Hash, Hasher},
  io,
};

use takumi_core::{
  path_data::path_data,
  resources::image::{
    SvgGradient, SvgImageData, SvgLineCap, SvgLineJoin, SvgOp, SvgOpPaint, SvgSpreadMethod,
    SvgStrokeStyle, sniff_mime, to_data_url,
  },
  style::{Affine, FillRule, ImageScalingAlgorithm, ToCss},
};

use crate::{Frame, GradientStop, ImageKey, Rgba, SvgDocument, matrix_attr, num, push_opacity};

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
        SvgOpPaint::Pattern { ops, .. } => is_vector(ops),
        _ => true,
      }),
    _ => true,
  })
}

impl SvgDocument {
  /// Writes `ops`, each `Push*` as a `<g>` its `Pop` closes.
  pub(crate) fn svg_ops(&mut self, ops: &[SvgOp]) -> io::Result<()> {
    let mut groups = Vec::new();

    for op in ops {
      match op {
        SvgOp::PushTransform(transform) => {
          groups.push(self.begin_group(*transform, 1.0, None, None)?);
        }
        SvgOp::PushClip { path, evenodd, .. } => {
          let clip = self.clip_path(
            &path_data(path, Affine::IDENTITY),
            fill_rule(*evenodd),
            None,
          )?;

          groups.push(self.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)?);
        }
        SvgOp::PushBlend { blend, .. } => {
          groups.push(self.begin_blend_group(&blend.to_css_string())?)
        }
        SvgOp::PushIsolation { .. } => groups.push(self.begin_isolate_group()?),
        SvgOp::PushOpacity { opacity, .. } => {
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
        SvgOp::Draw {
          path, fill, stroke, ..
        } => {
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
        // `is_vector` keeps filters out, but the matching `Pop` still closes a group.
        SvgOp::PushFilter(_) => {
          groups.push(self.begin_group(Affine::IDENTITY, 1.0, None, None)?);
        }
        SvgOp::Image {
          data,
          width,
          height,
          rendering,
        } => {
          let key = ImageKey::SvgBitmap(digest(data));
          let Some(href) = self.image_href(&key, || image_href(data)) else {
            continue;
          };
          let rect = Frame::new(0.0, 0.0, *width, *height);

          if *rendering == ImageScalingAlgorithm::Pixelated {
            let mut attrs = rect.attrs();

            attrs.push(("href", href.as_ref().into()));
            attrs.push(("image-rendering", "pixelated".into()));
            self.empty("image", &attrs)?;
          } else {
            self.image_use(&key, &href, (*width, *height), rect)?;
          }
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
  fn svg_paint(&mut self, paint: &SvgOpPaint) -> io::Result<String> {
    match paint {
      SvgOpPaint::Color(color) => Ok(Rgba(color.0).hex()),
      SvgOpPaint::Linear {
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
      SvgOpPaint::Radial {
        center,
        radius,
        focal,
        focal_radius,
        gradient,
      } => {
        let mut geometry = vec![
          ("cx", num(center.x).into()),
          ("cy", num(center.y).into()),
          ("r", num(*radius).into()),
          ("fx", num(focal.x).into()),
          ("fy", num(focal.y).into()),
        ];

        if *focal_radius != 0.0 {
          geometry.push(("fr", num(*focal_radius).into()));
        }
        self.svg_gradient("radialGradient", "rg", geometry, gradient)
      }
      SvgOpPaint::Pattern {
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
            ("patternTransform", matrix_attr(*transform).into()),
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
    let transform = gradient.transform;
    let stops: Vec<GradientStop> = gradient
      .stops
      .iter()
      .map(|stop| {
        let [red, green, blue, _] = stop.color.0;

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
/// A digest of `data`'s bytes or pixels, which keys its one encoding.
fn digest(data: &SvgImageData) -> u64 {
  let mut hasher = DefaultHasher::new();

  match data {
    SvgImageData::Encoded(bytes) => bytes.hash(&mut hasher),
    SvgImageData::Bitmap(buffer) => {
      (buffer.width(), buffer.height(), buffer.data()).hash(&mut hasher)
    }
  }
  hasher.finish()
}

fn image_href(data: &SvgImageData) -> Option<String> {
  match data {
    SvgImageData::Encoded(bytes) => Some(to_data_url(sniff_mime(bytes), bytes)),
    SvgImageData::Bitmap(buffer) => buffer
      .encode_png()
      .map(|png| to_data_url("image/png", &png)),
  }
}

fn fill_rule(evenodd: bool) -> FillRule {
  if evenodd {
    FillRule::EvenOdd
  } else {
    FillRule::NonZero
  }
}
