//! A scene flattened into paint chunks in paint order, and the conversion that moves a device
//! between their clip and effect states, after Blink's `PaintChunk` and the `ConversionContext` in
//! [`paint_chunks_to_cc_layer.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/graphics/compositing/paint_chunks_to_cc_layer.cc).

// `ConversionContext` follows Blink, under the notice in LICENSE-CHROMIUM.

use crate::{
  paint_property::{ClipId, ClipNode, EffectId, EffectNode, PropertyState, PropertyTrees},
  scene::{BoxPart, NodePaint, PaintItemKind, PaintPhase, SceneBounds, StackingContextNode},
};

/// Which draws of a box a chunk holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkPart {
  /// Its shadows, background and border.
  Decorations,
  /// Its text and replaced content.
  Content,
  /// Its outline.
  Outline,
  /// A table's collapsed borders.
  CollapsedBorders,
}

/// Part of one box, painted under one property state.
#[derive(Clone, Copy)]
pub struct PaintChunk<'s> {
  /// The box.
  pub node: &'s NodePaint,
  /// Which of its draws.
  pub part: ChunkPart,
}

impl<'s> PaintChunk<'s> {
  /// Every chunk of `contexts` in paint order: a stacking context's root decorations, its phases,
  /// then the outlines of its items and last its root's.
  pub fn in_paint_order(contexts: &'s [StackingContextNode]) -> Vec<Self> {
    let mut chunks = Vec::new();

    push_context_chunks(contexts, 0, &mut chunks);

    chunks
  }

  /// The box that owns each effect of `trees`, by effect index.
  pub fn effect_owners(chunks: &[Self], trees: &PropertyTrees) -> Vec<Option<&'s NodePaint>> {
    let mut owners = vec![None; trees.effect_count()];

    for chunk in chunks {
      for id in trees.effect_chain(chunk.node.properties.border_box.effect) {
        if trees.effect(id).owner != chunk.node.path {
          break;
        }

        owners[id.index()] = Some(chunk.node);
      }
    }

    owners
  }

  /// The clip and effect the chunk paints under.
  pub fn state(&self) -> PropertyState {
    match self.part {
      ChunkPart::Content | ChunkPart::CollapsedBorders => self.node.properties.contents,
      ChunkPart::Decorations | ChunkPart::Outline => self.node.properties.border_box,
    }
  }
}

fn push_context_chunks<'s>(
  contexts: &'s [StackingContextNode],
  context_id: usize,
  chunks: &mut Vec<PaintChunk<'s>>,
) {
  let Some(context) = contexts.get(context_id) else {
    return;
  };
  let roots = context.roots();
  let mut outlines = Vec::new();
  let chunk = |node, part| PaintChunk { node, part };

  chunks.extend(roots.iter().map(|root| chunk(root, ChunkPart::Decorations)));

  for phase in context.paint_phases() {
    match phase {
      PaintPhase::RootContent => {
        chunks.extend(roots.iter().map(|root| chunk(root, ChunkPart::Content)));
      }
      PaintPhase::Items(items, phase_part) => {
        for item in items {
          let Some(part) = item.part_in(phase_part) else {
            continue;
          };

          match &item.kind {
            PaintItemKind::Node(node) => {
              if part != BoxPart::Content {
                chunks.push(chunk(node, ChunkPart::Decorations));
              }
              if part != BoxPart::Decorations {
                chunks.push(chunk(node, ChunkPart::Content));
                outlines.push(chunk(node, ChunkPart::Outline));
              }
            }
            PaintItemKind::Context(child) => push_context_chunks(contexts, *child, chunks),
            PaintItemKind::CollapsedBorders(node) => {
              chunks.push(chunk(node, ChunkPart::CollapsedBorders));
            }
          }
        }
      }
    }
  }

  chunks.extend(outlines);
  chunks.extend(roots.iter().map(|root| chunk(root, ChunkPart::Outline)));
}

/// What a device does to enter and leave clips and effects.
pub trait PropertySink {
  /// Clips later draws to `clip`.
  fn push_clip(&mut self, id: ClipId, clip: &ClipNode);

  /// Removes the most recent clip.
  fn pop_clip(&mut self, clip: &ClipNode);

  /// Draws what follows into the group `effect` composites.
  fn begin_effect(&mut self, id: EffectId, effect: &EffectNode);

  /// Composites the most recent group.
  fn end_effect(&mut self);
}

/// What entering a clip or an effect replaced.
enum Entered {
  Clip {
    id: ClipId,
    previous: Option<ClipId>,
  },
  Effect {
    previous_clip: Option<ClipId>,
    previous: Option<EffectId>,
  },
}

/// Moves a [`PropertySink`] from one chunk's state to the next, as Blink's `ConversionContext`
/// does, entering and leaving as few clips and effects as it can.
pub struct ConversionContext<'t, S> {
  trees: &'t PropertyTrees,
  sink: S,
  clip: Option<ClipId>,
  effect: Option<EffectId>,
  stack: Vec<Entered>,
}

impl<'t, S: PropertySink> ConversionContext<'t, S> {
  /// Starts outside every clip and effect of `trees`.
  pub fn new(trees: &'t PropertyTrees, sink: S) -> Self {
    Self {
      trees,
      sink,
      clip: None,
      effect: None,
      stack: Vec::new(),
    }
  }

  /// The sink.
  pub fn sink(&mut self) -> &mut S {
    &mut self.sink
  }

  /// Enters `state`: its effect first, then its clip.
  pub fn switch_to(&mut self, state: PropertyState) {
    self.switch_to_effect(state.effect);
    self.switch_to_clip(state.clip);
  }

  /// Leaves every clip and effect and returns the sink.
  pub fn finish(mut self) -> S {
    while let Some(entered) = self.stack.pop() {
      match entered {
        Entered::Clip { id, .. } => self.sink.pop_clip(self.trees.clip(id)),
        Entered::Effect { .. } => self.sink.end_effect(),
      }
    }

    self.sink
  }

  /// Blink's `SwitchToClip`. A clip the current effect opened inside stays: a chunk cannot leave
  /// the clips its effect composites under.
  fn switch_to_clip(&mut self, target: Option<ClipId>) {
    if target == self.clip {
      return;
    }

    let common = self.trees.common_clip(target, self.clip);

    while self.clip != common && matches!(self.stack.last(), Some(Entered::Clip { .. })) {
      self.end_clip();
    }

    if target == self.clip {
      return;
    }

    let pending: Vec<ClipId> = self
      .trees
      .clip_chain(target)
      .take_while(|&id| Some(id) != self.clip)
      .collect();

    for id in pending.into_iter().rev() {
      self.sink.push_clip(id, self.trees.clip(id));
      self.stack.push(Entered::Clip {
        id,
        previous: self.clip,
      });
      self.clip = Some(id);
    }
  }

  /// Blink's `SwitchToEffect`.
  fn switch_to_effect(&mut self, target: Option<EffectId>) {
    if target == self.effect {
      return;
    }

    let common = self.trees.common_effect(target, self.effect);

    while self.effect != common {
      self.end_clips();

      if self.stack.is_empty() {
        break;
      }

      self.end_effect();
    }

    let pending: Vec<EffectId> = self
      .trees
      .effect_chain(target)
      .take_while(|&id| Some(id) != common)
      .collect();

    for id in pending.into_iter().rev() {
      self.start_effect(id);
    }
  }

  /// Blink's `StartEffect`: enters the effect's output clip, or leaves every clip when it has
  /// none, then opens the group.
  fn start_effect(&mut self, id: EffectId) {
    let effect = self.trees.effect(id);

    match effect.output_clip {
      Some(clip) => self.switch_to_clip(Some(clip)),
      None => self.end_clips(),
    }

    self.sink.begin_effect(id, effect);
    self.stack.push(Entered::Effect {
      previous_clip: self.clip,
      previous: self.effect,
    });
    self.effect = Some(id);
  }

  fn end_clip(&mut self) {
    if let Some(Entered::Clip { id, previous }) = self.stack.pop() {
      self.sink.pop_clip(self.trees.clip(id));
      self.clip = previous;
    }
  }

  fn end_clips(&mut self) {
    while matches!(self.stack.last(), Some(Entered::Clip { .. })) {
      self.end_clip();
    }
  }

  fn end_effect(&mut self) {
    if let Some(Entered::Effect {
      previous_clip,
      previous,
    }) = self.stack.pop()
    {
      self.sink.end_effect();
      self.clip = previous_clip;
      self.effect = previous;
    }
  }
}

/// The content height one index bucket covers, before a sparse scene widens it.
const BUCKET_HEIGHT: usize = 1024;

/// The most buckets one chunk enters. A taller one joins the chunks every
/// window checks, which keeps the index linear in its chunks.
const MAX_SPAN: usize = 16;

/// Chunk indices by the content height their paint bounds reach.
pub struct HeightIndex {
  /// Per band of `bucket_height` content height, the chunks reaching it.
  buckets: Vec<Vec<usize>>,
  bucket_height: usize,
  /// Chunks every window checks: those without bounds, and those too tall to bucket.
  unindexed: Vec<usize>,
}

/// The bucket row of device-space `y`; the rows start at the canvas top, which also holds what
/// lies above it, as [`HeightIndex::candidates`] clamps its queries there.
fn row(y: i32) -> usize {
  y.max(0) as usize
}

impl HeightIndex {
  /// The index of chunks whose paint reaches `bounds`, one each in paint order.
  pub fn new(bounds: &[Option<SceneBounds>]) -> Self {
    let bottom = bounds
      .iter()
      .flatten()
      .map(|bounds| row(bounds.bottom))
      .max()
      .unwrap_or_default();
    // A sparse scene widens its buckets so their count stays within its chunk count.
    let bucket_height = BUCKET_HEIGHT.max(bottom / (bounds.len() + 1) + 1);
    let mut buckets = vec![Vec::new(); bottom / bucket_height + 1];
    let mut unindexed = Vec::new();

    for (index, bounds) in bounds.iter().enumerate() {
      let span = bounds
        .map(|bounds| row(bounds.top) / bucket_height..=row(bounds.bottom) / bucket_height)
        .filter(|span| {
          span
            .end()
            .checked_sub(*span.start())
            .is_some_and(|reach| reach < MAX_SPAN)
        });

      match span {
        Some(span) => {
          for bucket in &mut buckets[span] {
            bucket.push(index);
          }
        }
        None => unindexed.push(index),
      }
    }

    Self {
      buckets,
      bucket_height,
      unindexed,
    }
  }

  /// The chunks that may reach `[top, bottom)`, in paint order, with every unindexed one.
  pub fn candidates(&self, top: f32, bottom: f32) -> Vec<usize> {
    let bucket = |y: f32| (y.max(0.0) as usize / self.bucket_height).min(self.buckets.len() - 1);
    let mut indices: Vec<usize> = self
      .buckets
      .get(bucket(top)..=bucket(bottom))
      .unwrap_or_default()
      .iter()
      .flatten()
      .chain(&self.unindexed)
      .copied()
      .collect();

    indices.sort_unstable();
    indices.dedup();
    indices
  }
}

#[cfg(test)]
mod tests {
  use super::{ConversionContext, HeightIndex, PropertySink};
  use crate::{
    geometry::Size,
    paint_property::{
      ClipId, ClipKind, ClipNode, EffectId, EffectKind, EffectNode, PropertyState, PropertyTrees,
    },
    painter::FillShape,
    scene::SceneBounds,
    style::Affine,
  };

  /// Writes each call down.
  #[derive(Default)]
  struct Log(Vec<String>);

  impl PropertySink for Log {
    fn push_clip(&mut self, id: ClipId, _clip: &ClipNode) {
      self.0.push(format!("clip {id:?}"));
    }

    fn pop_clip(&mut self, _clip: &ClipNode) {
      self.0.push("pop".into());
    }

    fn begin_effect(&mut self, id: EffectId, _effect: &EffectNode) {
      self.0.push(format!("effect {id:?}"));
    }

    fn end_effect(&mut self) {
      self.0.push("end".into());
    }
  }

  fn clip(trees: &mut PropertyTrees, parent: Option<ClipId>) -> ClipId {
    trees.add_clip(ClipNode {
      parent,
      transform: Affine::IDENTITY,
      kind: ClipKind::Shape(FillShape::Rect(Size {
        width: 1.0,
        height: 1.0,
      })),
      owner: Vec::new(),
    })
  }

  fn effect(
    trees: &mut PropertyTrees,
    parent: Option<EffectId>,
    output_clip: Option<ClipId>,
  ) -> EffectId {
    trees.add_effect(EffectNode {
      kind: EffectKind::Group,
      parent,
      output_clip,
      owner: Vec::new(),
      bounds: None,
    })
  }

  fn run(trees: &PropertyTrees, states: &[PropertyState]) -> Vec<String> {
    let mut conversion = ConversionContext::new(trees, Log::default());

    for &state in states {
      conversion.switch_to(state);
    }

    conversion.finish().0
  }

  fn state(clip: Option<ClipId>, effect: Option<EffectId>) -> PropertyState {
    PropertyState { clip, effect }
  }

  #[test]
  fn sibling_clips_leave_to_their_common_ancestor() {
    let mut trees = PropertyTrees::default();
    let outer = clip(&mut trees, None);
    let first = clip(&mut trees, Some(outer));
    let second = clip(&mut trees, Some(outer));

    assert_eq!(
      run(
        &trees,
        &[state(Some(first), None), state(Some(second), None)]
      ),
      [
        "clip ClipId(0)",
        "clip ClipId(1)",
        "pop",
        "clip ClipId(2)",
        "pop",
        "pop"
      ]
    );
  }

  #[test]
  fn an_effect_opens_inside_its_output_clip() {
    let mut trees = PropertyTrees::default();
    let outer = clip(&mut trees, None);
    let inner = clip(&mut trees, Some(outer));
    let group = effect(&mut trees, None, Some(outer));

    assert_eq!(
      run(&trees, &[state(Some(inner), Some(group))]),
      [
        "clip ClipId(0)",
        "effect EffectId(0)",
        "clip ClipId(1)",
        "pop",
        "end",
        "pop"
      ]
    );
  }

  #[test]
  fn a_chunk_cannot_leave_the_clip_its_effect_opened_in() {
    let mut trees = PropertyTrees::default();
    let outer = clip(&mut trees, None);
    let group = effect(&mut trees, None, Some(outer));

    assert_eq!(
      run(
        &trees,
        &[state(Some(outer), Some(group)), state(None, Some(group))]
      ),
      ["clip ClipId(0)", "effect EffectId(0)", "end", "pop"]
    );
  }

  #[test]
  fn an_effect_without_output_clip_opens_outside_every_clip() {
    let mut trees = PropertyTrees::default();
    let outer = clip(&mut trees, None);
    let group = effect(&mut trees, None, None);

    assert_eq!(
      run(
        &trees,
        &[
          state(Some(outer), None),
          state(Some(outer), Some(group)),
          state(None, Some(group))
        ]
      ),
      [
        "clip ClipId(0)",
        "pop",
        "effect EffectId(0)",
        "clip ClipId(0)",
        "pop",
        "end"
      ]
    );
  }

  fn bounds(top: i32, bottom: i32) -> Option<SceneBounds> {
    Some(SceneBounds {
      left: 0,
      top,
      right: 10,
      bottom,
    })
  }

  #[test]
  fn a_window_finds_the_chunks_it_reaches() {
    let index = HeightIndex::new(&[bounds(0, 10), bounds(5000, 5010), None, bounds(9000, 9010)]);

    assert_eq!(index.candidates(4800.0, 5200.0), [1, 2]);
  }

  #[test]
  fn an_inverted_window_finds_only_the_unindexed_chunks() {
    let index = HeightIndex::new(&[bounds(0, 10), None, bounds(9000, 9010)]);

    assert_eq!(index.candidates(9000.0, 100.0), [1]);
  }

  #[test]
  fn tall_chunks_stay_out_of_the_buckets() {
    let tall = vec![bounds(0, 1_000_000); 1000];
    let index = HeightIndex::new(&tall);

    assert_eq!(index.buckets.iter().map(Vec::len).sum::<usize>(), 0);
    assert_eq!(index.unindexed.len(), 1000);
  }

  #[test]
  fn inverted_bounds_stay_out_of_the_buckets() {
    let index = HeightIndex::new(&[bounds(5000, 10)]);

    assert_eq!(index.unindexed, [0]);
  }
}
