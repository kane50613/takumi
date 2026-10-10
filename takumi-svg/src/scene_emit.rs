//! Scene-driven SVG emission: the paint chunks takumi-core flattens a scene into — the same paint
//! order, clips and effects the raster backend consumes — written as nested groups.
//!
//! Each chunk is placed by its transform relative to the groups open around it, so a pure
//! translation folds into the draw origin to keep the output compact, and a rotation or scale
//! becomes a group's `transform`.

use std::io;

use takumi_core::{
  geometry::Point,
  layout::inline::InlinePass,
  paint_chunk::{ChunkPart, ConversionContext, PaintChunk, PropertySink},
  paint_property::{ClipId, ClipNode, EffectId, EffectNode},
  painter::{BoxFrame, TextClip},
  scene::{NodePaint, Scene},
  style::{Affine, Filter},
};

use crate::{
  GroupToken, SvgDocument,
  render::{EffectGroups, PlacedBox},
};

/// A scene, emitted in paint order.
pub(crate) struct SceneEmitter<'a> {
  pub(crate) scene: &'a Scene,
}

impl<'a> SceneEmitter<'a> {
  pub(crate) fn emit(&self, doc: &mut SvgDocument) -> io::Result<()> {
    let chunks = PaintChunk::in_paint_order(&self.scene.contexts);
    let owners = PaintChunk::effect_owners(&chunks, &self.scene.properties);

    self.emit_chunks(&chunks, &chunks, &owners, true, doc)
  }

  /// Emits `emitted`, a prefix of the scene's `chunks`, entering their clips and effects. Only a
  /// top-level pass replays backdrops, since a replay would replay again for each nested one.
  fn emit_chunks(
    &self,
    chunks: &[PaintChunk<'a>],
    emitted: &[PaintChunk<'a>],
    owners: &[Option<&'a NodePaint>],
    backdrops: bool,
    doc: &mut SvgDocument,
  ) -> io::Result<()> {
    let base = doc.transform();
    let mut conversion = ConversionContext::new(
      &self.scene.properties,
      ChunkWriter {
        emitter: self,
        chunks,
        owners,
        backdrops,
        base,
        doc,
        groups: Vec::new(),
        error: None,
      },
    );

    for chunk in emitted {
      conversion.switch_to(chunk.state());
      conversion.sink().emit(chunk);
    }

    conversion.finish().error.map_or(Ok(()), Err)
  }

  /// The box `paint` names, placed relative to `current`, with the transform its group needs
  /// beyond a translation.
  fn place(&self, paint: &NodePaint, current: Affine) -> Option<(PlacedBox<'a>, Affine)> {
    let node = self.scene.root.node_at_path(&paint.path)?;
    let layout = self.scene.results.layout(paint.node_id).ok()?;
    let relative = current.invert().unwrap_or(Affine::IDENTITY) * paint.transform;
    let (origin, group_transform) = if relative.only_translation() {
      (
        Point {
          x: relative.x,
          y: relative.y,
        },
        Affine::IDENTITY,
      )
    } else {
      (Point::ZERO, relative)
    };

    Some((
      PlacedBox::new(node, BoxFrame::new(layout, origin)),
      group_transform,
    ))
  }
}

/// A [`PropertySink`] writing chunks into an [`SvgDocument`].
struct ChunkWriter<'e, 'a, 'd> {
  emitter: &'e SceneEmitter<'a>,
  chunks: &'e [PaintChunk<'a>],
  owners: &'e [Option<&'a NodePaint>],
  backdrops: bool,
  /// The document transform the scene's transforms are relative to.
  base: Affine,
  doc: &'d mut SvgDocument,
  groups: Vec<Vec<GroupToken>>,
  error: Option<io::Error>,
}

impl ChunkWriter<'_, '_, '_> {
  /// The transform the open groups add to the scene's space.
  fn current(&self) -> Affine {
    self.base.invert().unwrap_or(Affine::IDENTITY) * self.doc.transform()
  }

  fn record(&mut self, result: io::Result<()>) {
    if let Err(error) = result {
      self.error.get_or_insert(error);
    }
  }

  /// Writes one chunk under the groups already open.
  fn emit(&mut self, chunk: &PaintChunk<'_>) {
    if self.error.is_some() {
      return;
    }

    let Some((placed, group_transform)) = self.emitter.place(chunk.node, self.current()) else {
      return;
    };
    let doc = &mut *self.doc;
    let result = (|| {
      let group = (!group_transform.is_identity())
        .then(|| doc.begin_group(group_transform, 1.0, None, None))
        .transpose()?;

      match chunk.part {
        ChunkPart::Decorations => {
          let scene = self.emitter.scene;
          let text_clip =
            TextClip::of(&scene.root, &scene.results, chunk.node).map_err(io::Error::other)?;

          placed.emit_decorations(text_clip.as_ref(), doc)?;
        }
        ChunkPart::Content => placed.emit_own_content(InlinePass::Content, doc)?,
        ChunkPart::Floats => placed.emit_own_content(InlinePass::Floats, doc)?,
        ChunkPart::Outline => placed.emit_outline(doc)?,
      }

      match group {
        Some(group) => doc.end_group(group),
        None => Ok(()),
      }
    })();

    self.record(result);
  }

  /// Opens the groups of the effect `paint` owns, after its filtered backdrop.
  fn open_effect(&mut self, paint: &NodePaint) -> io::Result<Vec<GroupToken>> {
    let Some((placed, group_transform)) = self.emitter.place(paint, self.current()) else {
      return Ok(Vec::new());
    };

    if self.backdrops
      && !placed
        .node
        .context
        .style
        .misc1_data
        .backdrop_filter
        .is_empty()
    {
      self.emit_backdrop(&placed, paint, group_transform)?;
    }

    Ok(EffectGroups::open(&placed, group_transform, self.doc)?.into_tokens())
  }

  /// Emits `placed`'s backdrop: the chunks before its own, filtered and clipped to its border box,
  /// since SVG has no backdrop source of its own.
  fn emit_backdrop(
    &mut self,
    placed: &PlacedBox,
    paint: &NodePaint,
    group_transform: Affine,
  ) -> io::Result<()> {
    let context = &placed.node.context;
    let size = placed.frame.layout.size;
    let filters: Vec<Filter> = context
      .style
      .misc1_data
      .backdrop_filter
      .iter()
      .filter(|f| !f.is_drop_shadow())
      .cloned()
      .collect();

    if filters.is_empty() || size.width <= 0.0 || size.height <= 0.0 {
      return Ok(());
    }

    let doc = &mut *self.doc;
    let outer = (!group_transform.is_identity())
      .then(|| doc.begin_group(group_transform, 1.0, None, None))
      .transpose()?;
    let clip_group = doc.begin_clipped_group(&placed.border_box_path_data())?;
    let shape_clip = placed.begin_clip_path_group(doc)?;
    let mask = placed.begin_mask_group(doc)?;
    // The blur feathers the clipped backdrop's alpha at its edges; restoring it averages only the
    // pixels inside, close to the mirrored edges browsers sample. Skipped for opacity(), which
    // lowers alpha on purpose.
    let restore_alpha = !filters.iter().any(|f| matches!(f, Filter::Opacity(_)));
    let filter_refs = doc.filter(&filters, context, size, restore_alpha)?;
    let filter_wrappers = doc.begin_filter_wrappers(&filter_refs)?;
    let filter_group = doc.begin_group(
      Affine::IDENTITY,
      1.0,
      None,
      filter_refs.first().map(String::as_str),
    )?;
    // The filter reads only the backdrop inside the border box, as browsers do.
    let backdrop_clip = doc.begin_clipped_group(&placed.border_box_path_data())?;
    // The replay is emitted in the scene's space; cancel the groups open around it.
    let to_scene = (self.base.invert().unwrap_or(Affine::IDENTITY) * doc.transform())
      .invert()
      .unwrap_or(Affine::IDENTITY);
    let scene_group = (!to_scene.is_identity())
      .then(|| doc.begin_group(to_scene, 1.0, None, None))
      .transpose()?;
    let start = self
      .chunks
      .iter()
      .position(|chunk| chunk.node.path == paint.path)
      .unwrap_or(self.chunks.len());

    self
      .emitter
      .emit_chunks(self.chunks, &self.chunks[..start], self.owners, false, doc)?;

    if let Some(group) = scene_group {
      doc.end_group(group)?;
    }
    doc.end_group(backdrop_clip)?;
    doc.end_group(filter_group)?;
    doc.end_filter_wrappers(filter_wrappers)?;
    for group in [mask, shape_clip].into_iter().flatten() {
      doc.end_group(group)?;
    }
    doc.end_group(clip_group)?;
    if let Some(group) = outer {
      doc.end_group(group)?;
    }
    Ok(())
  }

  fn close(&mut self) {
    let Some(groups) = self.groups.pop() else {
      return;
    };
    let doc = &mut *self.doc;
    let result = groups
      .into_iter()
      .rev()
      .try_for_each(|group| doc.end_group(group));

    self.record(result);
  }
}

impl PropertySink for ChunkWriter<'_, '_, '_> {
  fn push_clip(&mut self, _id: ClipId, clip: &ClipNode) {
    let relative = self.current().invert().unwrap_or(Affine::IDENTITY) * clip.transform;
    let doc = &mut *self.doc;
    let opened = doc
      .clip_shape(&clip.shape, relative)
      .and_then(|reference| doc.begin_group(Affine::IDENTITY, 1.0, Some(&reference), None));

    match opened {
      Ok(group) => self.groups.push(vec![group]),
      Err(error) => {
        self.groups.push(Vec::new());
        self.record(Err(error));
      }
    }
  }

  fn pop_clip(&mut self) {
    self.close();
  }

  fn begin_effect(&mut self, id: EffectId, _effect: &EffectNode) {
    let opened = match self.owners[id.index()] {
      Some(owner) => self.open_effect(owner),
      None => Ok(Vec::new()),
    };

    match opened {
      Ok(groups) => self.groups.push(groups),
      Err(error) => {
        self.groups.push(Vec::new());
        self.record(Err(error));
      }
    }
  }

  fn end_effect(&mut self) {
    self.close();
  }
}
