//! Writes the laid out tree as drawing ops, after Blink's `SVGContainerPainter` and
//! `SVGObjectPainter`: a layer is pushed for each transform, clip, mask, blend mode, and opacity
//! a container has, and popped after its children.

use std::{iter::repeat_n, mem};

use super::{
  budget::Budget,
  object::{
    Container, Fill, LayoutClip, LayoutMask, LayoutObject, Picture, PictureContent, Shape, Stroke,
  },
};
#[cfg(feature = "svg-raster")]
use crate::resources::image::SvgFilters;
use crate::{
  geometry::{PathCommand, Point},
  resources::image::{SvgFill, SvgImageData, SvgOp, SvgStrokeStyle},
  style::{Affine, BlendMode},
};

/// Writes ops for the objects it is handed into one list, within a budget that charges a
/// resource again each time something references it.
pub(super) struct SvgPainter<'b> {
  ops: Vec<SvgOp>,
  budget: &'b mut Budget,
  exhausted: bool,
}

impl<'b> SvgPainter<'b> {
  /// The ops that draw `root` and everything below it, none when the budget runs out.
  pub(super) fn paint(root: &Container, budget: &'b mut Budget) -> Option<Vec<SvgOp>> {
    let mut painter = Self {
      ops: Vec::new(),
      budget,
      exhausted: false,
    };

    painter.container(root);
    (!painter.exhausted).then_some(painter.ops)
  }

  /// The ops `fill` writes into a fresh list: a mask, a clip mask or a pattern tile.
  fn nested(&mut self, fill: impl FnOnce(&mut Self)) -> Vec<SvgOp> {
    let outer = mem::take(&mut self.ops);

    fill(self);
    mem::replace(&mut self.ops, outer)
  }

  /// Appends an op, charging the budget for it and the path it carries.
  fn push(&mut self, op: SvgOp) {
    let weight = match &op {
      SvgOp::PushClip { path, .. } => 1 + path.len(),
      SvgOp::Draw { path, stroke, .. } => {
        let dashes = stroke
          .as_ref()
          .and_then(|stroke| stroke.dash.as_ref())
          .map_or(0, |(dashes, _)| dashes.len());

        1 + path.len() + dashes
      }
      SvgOp::Image {
        data: SvgImageData::Bitmap(buffer),
        ..
      } => 1 + buffer.data().len() / 64,
      _ => 1,
    };

    if self.exhausted || !self.budget.spend_painted(weight) {
      self.exhausted = true;
      return;
    }

    self.ops.push(op);
  }

  fn container(&mut self, container: &Container) {
    if self.exhausted {
      return;
    }

    let mut pop_count = 0;

    if container.transform.to_cols_array() != Affine::IDENTITY.to_cols_array() {
      self.push(SvgOp::PushTransform(container.transform));
      pop_count += 1;
    }

    if let Some(clip_path) = &container.clip_path {
      pop_count += self.push_clip_path(clip_path);
    }

    if let Some(mask) = &container.mask {
      let op = self.mask_op(mask);

      self.push(op);
      pop_count += 1;
    }

    if container.blend_mode != BlendMode::Normal {
      self.push(SvgOp::PushBlend(container.blend_mode));
      pop_count += 1;
    }

    if container.opacity < 1.0 {
      self.push(SvgOp::PushOpacity(container.opacity));
      pop_count += 1;
    }

    #[cfg(feature = "svg-raster")]
    if !container.filters.is_empty() {
      self.push(SvgOp::PushFilter(SvgFilters::new(
        container.filters.clone(),
      )));
      pop_count += 1;
    }

    for child in &container.children {
      self.object(child);
    }

    self.pop(pop_count);
  }

  /// Closes the innermost `count` open layers.
  fn pop(&mut self, count: usize) {
    for op in repeat_n(SvgOp::Pop, count) {
      self.push(op);
    }
  }

  fn object(&mut self, object: &LayoutObject) {
    match object {
      LayoutObject::Container(container) => self.container(container),
      LayoutObject::Shape(shape) => self.shape(shape),
      LayoutObject::Picture(picture) => self.picture(picture),
    }
  }

  fn shape(&mut self, shape: &Shape) {
    if !shape.visible || shape.commands.is_empty() {
      return;
    }

    let fill = shape.fill.as_ref().map(|fill| self.fill(fill));
    let stroke = shape.stroke.as_ref().map(|stroke| self.stroke(stroke));

    if fill.is_none() && stroke.is_none() {
      return;
    }

    let anti_alias = shape.anti_alias;

    if shape.stroke_first {
      if let Some(stroke) = stroke {
        self.push(SvgOp::Draw {
          path: shape.commands.clone(),
          fill: None,
          stroke: Some(stroke),
          anti_alias,
        });
      }

      if let Some(fill) = fill {
        self.push(SvgOp::Draw {
          path: shape.commands.clone(),
          fill: Some(fill),
          stroke: None,
          anti_alias,
        });
      }
    } else {
      self.push(SvgOp::Draw {
        path: shape.commands.clone(),
        fill,
        stroke,
        anti_alias,
      });
    }
  }

  fn picture(&mut self, picture: &Picture) {
    if !picture.visible {
      return;
    }

    match &picture.content {
      PictureContent::Svg(document) => {
        self.push(SvgOp::PushClip {
          path: PathCommand::rectangle(0.0, 0.0, document.width, document.height),
          evenodd: false,
        });
        self.container(&document.root);
        self.push(SvgOp::Pop);
      }
      PictureContent::Bitmap(data) => {
        if let Some(data) = data {
          self.push(SvgOp::Image {
            data: SvgImageData::clone(data),
            width: picture.width,
            height: picture.height,
            rendering: picture.rendering,
          });
        }
      }
    }
  }

  fn fill(&mut self, fill: &Fill) -> SvgFill {
    SvgFill {
      paint: fill
        .paint
        .to_op(|root| self.nested(|tile| tile.container(root))),
      opacity: fill.opacity,
      evenodd: fill.evenodd,
    }
  }

  fn stroke(&mut self, stroke: &Stroke) -> SvgStrokeStyle {
    SvgStrokeStyle {
      paint: stroke
        .paint
        .to_op(|root| self.nested(|tile| tile.container(root))),
      opacity: stroke.opacity,
      width: stroke.width,
      miter_limit: stroke.miter_limit,
      cap: stroke.cap,
      join: stroke.join,
      dash: stroke.dash.as_deref().cloned(),
    }
  }

  /// Emits clip layers for `clip_path`; returns how many layers were pushed.
  ///
  /// Simple clips (no nested clip on a child, uniform nonzero rules or a single even-odd shape)
  /// become native clip ops, anything else becomes an alpha mask, mirroring krilla-svg.
  fn push_clip_path(&mut self, clip_path: &LayoutClip) -> usize {
    let mut rules = Vec::new();

    collect_clip_rules(&clip_path.root, &mut rules);

    // Uniform nonzero rules always convert, even-odd only as a single shape (overlapping even-odd
    // shapes render differently in PDF).
    let simple = is_simple_clip_path(&clip_path.root)
      && match rules.as_slice() {
        [true] => true,
        rules => rules.iter().all(|evenodd| !evenodd),
      };

    if simple {
      let evenodd = rules.first().copied().unwrap_or(false);

      self.push_simple_clips(clip_path, evenodd)
    } else {
      let op = self.complex_clip_op(clip_path);

      self.push(op);
      1
    }
  }

  fn push_simple_clips(&mut self, clip_path: &LayoutClip, evenodd: bool) -> usize {
    let mut pushed = 0;

    if let Some(nested) = &clip_path.clip_path {
      pushed += self.push_simple_clips(nested, evenodd);
    }

    let mut commands = Vec::new();

    extend_clip_commands(&clip_path.root, clip_path.transform, &mut commands);

    if commands.is_empty() {
      // A clip path with only hidden children still hides everything.
      commands.push(PathCommand::MoveTo(Point::ZERO));
      commands.push(PathCommand::LineTo(Point::ZERO));
    }

    self.push(SvgOp::PushClip {
      path: commands,
      evenodd,
    });
    pushed + 1
  }

  fn complex_clip_op(&mut self, clip_path: &LayoutClip) -> SvgOp {
    let ops = self.nested(|mask| {
      let mut pop_count = 0;

      if let Some(nested) = &clip_path.clip_path {
        let op = mask.complex_clip_op(nested);

        mask.push(op);
        pop_count += 1;
      }

      if clip_path.transform.to_cols_array() != Affine::IDENTITY.to_cols_array() {
        mask.push(SvgOp::PushTransform(clip_path.transform));
        pop_count += 1;
      }

      mask.container(&clip_path.root);
      mask.pop(pop_count);
    });

    SvgOp::PushMask {
      ops,
      luminance: false,
    }
  }

  fn mask_op(&mut self, mask: &LayoutMask) -> SvgOp {
    let ops = self.nested(|painter| {
      let mut pop_count = 0;

      if let Some(nested) = &mask.mask {
        let op = painter.mask_op(nested);

        painter.push(op);
        pop_count += 1;
      }

      painter.push(SvgOp::PushClip {
        path: PathCommand::rectangle(
          mask.rect.x(),
          mask.rect.y(),
          mask.rect.width(),
          mask.rect.height(),
        ),
        evenodd: false,
      });
      pop_count += 1;
      painter.container(&mask.root);
      painter.pop(pop_count);
    });

    SvgOp::PushMask {
      ops,
      luminance: mask.luminance,
    }
  }
}

/// The fill rules of the shapes below `container`, `true` for even-odd.
fn collect_clip_rules(container: &Container, rules: &mut Vec<bool>) {
  for child in &container.children {
    match child {
      LayoutObject::Shape(shape) => {
        if let Some(fill) = &shape.fill {
          rules.push(fill.evenodd);
        }
      }
      LayoutObject::Container(container) => collect_clip_rules(container, rules),
      LayoutObject::Picture(_) => {}
    }
  }
}

fn is_simple_clip_path(container: &Container) -> bool {
  container.children.iter().all(|child| match child {
    LayoutObject::Container(container) => {
      container.clip_path.is_none() && is_simple_clip_path(container)
    }
    _ => true,
  })
}

fn extend_clip_commands(container: &Container, transform: Affine, commands: &mut Vec<PathCommand>) {
  for child in &container.children {
    match child {
      LayoutObject::Shape(shape) => {
        if shape.visible {
          commands.extend(shape.commands.iter().map(|command| {
            command.map_points(|point| {
              let (x, y) = transform.transform_point(point.x, point.y);

              Point::new(x, y)
            })
          }));
        }
      }
      LayoutObject::Container(child) => {
        let child_transform = transform * child.transform;

        extend_clip_commands(child, child_transform, commands);
      }
      LayoutObject::Picture(_) => {}
    }
  }
}
