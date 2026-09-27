//! A scene flattened into paint chunks in paint order, and the conversion that moves a device
//! between their clip and effect states, after Blink's `PaintChunk` and the `ConversionContext` in
//! [`paint_chunks_to_cc_layer.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/graphics/compositing/paint_chunks_to_cc_layer.cc).

// `ConversionContext` follows Blink, under the notice in LICENSE-CHROMIUM.

use crate::{
  paint_property::{ClipId, ClipNode, EffectId, EffectNode, PropertyState, PropertyTrees},
  scene::{BoxPart, NodePaint, PaintItemKind, PaintPhase, StackingContextNode},
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
      if let Some(id) = chunk.node.properties.border_box.effect
        && trees.effect(id).owner == chunk.node.path
      {
        owners[id.index()] = Some(chunk.node);
      }
    }

    owners
  }

  /// The clip and effect the chunk paints under.
  pub fn state(&self) -> PropertyState {
    match self.part {
      ChunkPart::Content => self.node.properties.contents,
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
  let root = context.root();
  let mut outlines = Vec::new();
  let chunk = |node, part| PaintChunk { node, part };

  if let Some(root) = root {
    chunks.push(chunk(root, ChunkPart::Decorations));
  }

  for phase in context.paint_phases() {
    match phase {
      PaintPhase::RootContent => chunks.extend(root.map(|root| chunk(root, ChunkPart::Content))),
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
          }
        }
      }
    }
  }

  chunks.extend(outlines);
  chunks.extend(root.map(|root| chunk(root, ChunkPart::Outline)));
}

/// What a device does to enter and leave clips and effects.
pub trait PropertySink {
  /// Clips later draws to `clip`.
  fn push_clip(&mut self, id: ClipId, clip: &ClipNode);

  /// Removes the most recent clip.
  fn pop_clip(&mut self);

  /// Draws what follows into the group `effect` composites.
  fn begin_effect(&mut self, id: EffectId, effect: &EffectNode);

  /// Composites the most recent group.
  fn end_effect(&mut self);
}

/// What entering a clip or an effect replaced.
enum Entered {
  Clip {
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
        Entered::Clip { .. } => self.sink.pop_clip(),
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
    if let Some(Entered::Clip { previous }) = self.stack.pop() {
      self.sink.pop_clip();
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

#[cfg(test)]
mod tests {
  use super::{ConversionContext, PropertySink};
  use crate::{
    geometry::Size,
    paint_property::{ClipId, ClipNode, EffectId, EffectNode, PropertyState, PropertyTrees},
    painter::FillShape,
    style::Affine,
  };

  /// Writes each call down.
  #[derive(Default)]
  struct Log(Vec<String>);

  impl PropertySink for Log {
    fn push_clip(&mut self, id: ClipId, _clip: &ClipNode) {
      self.0.push(format!("clip {id:?}"));
    }

    fn pop_clip(&mut self) {
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
      shape: FillShape::Rect(Size {
        width: 1.0,
        height: 1.0,
      }),
      owner: Vec::new(),
    })
  }

  fn effect(
    trees: &mut PropertyTrees,
    parent: Option<EffectId>,
    output_clip: Option<ClipId>,
  ) -> EffectId {
    trees.add_effect(EffectNode {
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
}
