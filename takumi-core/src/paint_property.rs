//! Clip and effect trees every painted box refers to, after Blink's
//! [paint property trees](https://chromium.googlesource.com/chromium/src/+/main/third_party/blink/renderer/core/paint/README.md#paint-property-trees).
//!
//! Transforms stay accumulated on each box, so only clips and effects form trees here. A box's
//! decorations paint under its border-box state and its content under its contents state, the way
//! Blink's `PaintPropertyTreeBuilder` hands out `LocalBorderBoxProperties` and
//! `ContentsProperties`.

// The property rules follow Blink, under the notice in LICENSE-CHROMIUM.

use std::iter::successors;

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout, Point},
  painter::{FillShape, OverflowClip},
  scene::SceneBounds,
  style::Affine,
};

/// A clip in [`PropertyTrees`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipId(usize);

/// An effect in [`PropertyTrees`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectId(usize);

impl EffectId {
  /// The effect's position in its trees.
  pub fn index(self) -> usize {
    self.0
  }
}

/// A region later draws stay inside.
#[derive(Debug, Clone)]
pub struct ClipNode {
  /// The clip this one sits inside, or `None` under the root.
  pub parent: Option<ClipId>,
  /// Where the shape sits.
  pub transform: Affine,
  /// The region.
  pub shape: FillShape,
  /// Child-index path from the root to the box whose `overflow` the clip is.
  pub owner: Vec<usize>,
}

/// A group composited as one, such as `opacity` or `filter`.
#[derive(Debug, Clone)]
pub struct EffectNode {
  /// The effect this one composites into, or `None` under the root.
  pub parent: Option<EffectId>,
  /// The clip the group composites under, or `None` when a descendant escapes the clips the
  /// owner sits in and the group has to open outside all of them.
  pub output_clip: Option<ClipId>,
  /// Child-index path from the root to the box that owns the effect.
  pub owner: Vec<usize>,
  /// Device-space bounds of what the group paints, filters included, or `None` when unknown.
  pub bounds: Option<SceneBounds>,
}

/// The clip and effect a draw paints under.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PropertyState {
  /// The innermost clip, or `None` for none.
  pub clip: Option<ClipId>,
  /// The innermost effect, or `None` for none.
  pub effect: Option<EffectId>,
}

/// The states one box paints under.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NodeProperties {
  /// For its background, border, shadows and outline.
  pub border_box: PropertyState,
  /// For its content and children.
  pub contents: PropertyState,
}

impl NodeProperties {
  /// Adds the effect and overflow clip of the box at `path` to `trees`, starting from `state`, as
  /// Blink's `UpdateForSelf` and `UpdateForChildren` do. Its whole paint composites as one group
  /// when it needs one.
  pub(crate) fn build(
    trees: &mut PropertyTrees,
    state: PropertyState,
    path: &[usize],
    context: &RenderContext,
    layout: ComputedLayout,
    transform: Affine,
    paint_offset: Point<f32>,
  ) -> Self {
    let effect = if context.style.needs_offscreen_compositing() {
      Some(trees.add_effect(EffectNode {
        parent: state.effect,
        output_clip: state.clip,
        owner: path.to_vec(),
        bounds: None,
      }))
    } else {
      state.effect
    };
    let clip = OverflowClip::of(context, layout, paint_offset)
      .map(|clip| {
        let (shape, origin) = clip.shape();

        trees.add_clip(ClipNode {
          parent: state.clip,
          transform: transform * Affine::translation(origin.x, origin.y),
          shape,
          owner: path.to_vec(),
        })
      })
      .or(state.clip);

    Self {
      border_box: PropertyState {
        clip: state.clip,
        effect,
      },
      contents: PropertyState { clip, effect },
    }
  }
}

/// What an out-of-flow box takes from its containing block.
pub(crate) struct ContainerContents {
  /// The clip the containing block's contents paint under.
  pub(crate) clip: Option<ClipId>,
  /// Child-index path from the root to the containing block.
  pub(crate) path: Vec<usize>,
}

/// The clip and effect trees of a scene.
#[derive(Debug, Clone, Default)]
pub struct PropertyTrees {
  clips: Vec<ClipNode>,
  effects: Vec<EffectNode>,
}

impl PropertyTrees {
  /// Adds a clip and returns it.
  pub(crate) fn add_clip(&mut self, clip: ClipNode) -> ClipId {
    self.clips.push(clip);
    ClipId(self.clips.len() - 1)
  }

  /// Adds an effect and returns it.
  pub(crate) fn add_effect(&mut self, effect: EffectNode) -> EffectId {
    self.effects.push(effect);
    EffectId(self.effects.len() - 1)
  }

  /// The clip `id` names.
  pub fn clip(&self, id: ClipId) -> &ClipNode {
    &self.clips[id.0]
  }

  /// The effect `id` names.
  pub fn effect(&self, id: EffectId) -> &EffectNode {
    &self.effects[id.0]
  }

  /// How many effects the trees hold.
  pub(crate) fn effect_count(&self) -> usize {
    self.effects.len()
  }

  /// Every effect, in the order they were added.
  pub(crate) fn effect_ids(&self) -> impl Iterator<Item = EffectId> + use<> {
    (0..self.effects.len()).map(EffectId)
  }

  /// Sets the bounds of the effect `id`.
  pub(crate) fn set_effect_bounds(&mut self, id: EffectId, bounds: Option<SceneBounds>) {
    self.effects[id.0].bounds = bounds;
  }

  /// Lets every effect from `effect` up to `container` open outside all clips when an
  /// out-of-flow descendant paints under the container's clip rather than theirs, as Blink's
  /// `EffectCanUseCurrentClipAsOutputClip` decides.
  pub(crate) fn release_escaped_effects(
    &mut self,
    effect: Option<EffectId>,
    container: &ContainerContents,
  ) {
    let escaped: Vec<EffectId> = self
      .effect_chain(effect)
      .take_while(|&id| !container.path.starts_with(&self.effect(id).owner))
      .filter(|&id| self.effect(id).output_clip != container.clip)
      .collect();

    for id in escaped {
      self.effects[id.0].output_clip = None;
    }
  }

  /// Whether `ancestor` is `clip` or contains it.
  pub(crate) fn clip_contains(&self, ancestor: Option<ClipId>, clip: Option<ClipId>) -> bool {
    let Some(ancestor) = ancestor else {
      return true;
    };

    self.clip_chain(clip).any(|id| id == ancestor)
  }

  /// The deepest clip containing both `a` and `b`.
  pub(crate) fn common_clip(&self, a: Option<ClipId>, b: Option<ClipId>) -> Option<ClipId> {
    self
      .clip_chain(a)
      .find(|&id| self.clip_contains(Some(id), b))
  }

  /// The deepest effect containing both `a` and `b`.
  pub(crate) fn common_effect(&self, a: Option<EffectId>, b: Option<EffectId>) -> Option<EffectId> {
    self
      .effect_chain(a)
      .find(|&id| self.effect_chain(b).any(|other| other == id))
  }

  /// `clip` and its ancestors, innermost first.
  pub(crate) fn clip_chain(&self, clip: Option<ClipId>) -> impl Iterator<Item = ClipId> + '_ {
    successors(clip, |&id| self.clip(id).parent)
  }

  /// `effect` and its ancestors, innermost first.
  pub(crate) fn effect_chain(
    &self,
    effect: Option<EffectId>,
  ) -> impl Iterator<Item = EffectId> + '_ {
    successors(effect, |&id| self.effect(id).parent)
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::NodeProperties;
  use crate::{
    context::RenderContext,
    layout::{node::Node, tree::RenderNode},
    resources::font::Fonts,
    scene::{PaintItemKind, Scene, StackingContextNode},
    style::{SizingContext, StyleSheet},
    viewport::Viewport,
  };

  const STYLES: &str = r#"
    * { display: block; width: 40px; height: 40px; }
    #positioned { position: relative; }
    #clip { overflow: hidden; }
    #positioned-clip { position: relative; overflow: hidden; }
    #faded { opacity: 0.5; }
    #absolute { position: absolute; }
  "#;

  /// Lays out `node` under [`STYLES`].
  fn scene(node: Node) -> Scene {
    let fonts = Fonts::default();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((200, 200)))
          .build(),
      )
      .stylesheet(Arc::new(
        StyleSheet::parse(STYLES).expect("stylesheet parses"),
      ))
      .build();

    Scene::lay_out(
      RenderNode::from_node(&context, node),
      Viewport::new((200, 200)),
      false,
    )
    .expect("scene builds")
  }

  /// The properties of the box at `path`.
  fn properties(scene: &Scene, path: &[usize]) -> NodeProperties {
    let paints =
      |context: &StackingContextNode| {
        context
          .root()
          .into_iter()
          .chain(context.in_paint_order().into_iter().flatten().filter_map(
            |item| match &item.kind {
              PaintItemKind::Node(node) => Some(node),
              PaintItemKind::Context(_) | PaintItemKind::Floats(_) => None,
            },
          ))
          .find(|node| node.path == path)
          .map(|node| node.properties)
      };

    scene
      .contexts
      .iter()
      .find_map(paints)
      .expect("box is painted")
  }

  fn div<const N: usize>(id: &str, children: [Node; N]) -> Node {
    Node::container(children).with_id(id)
  }

  #[test]
  fn in_flow_children_paint_under_the_overflow_clip() {
    let scene = scene(div("root", [div("clip", [div("child", [])])]));
    let clip = properties(&scene, &[0]);
    let child = properties(&scene, &[0, 0]);

    assert!(clip.contents.clip.is_some());
    assert_eq!(clip.border_box.clip, None);
    assert_eq!(child.border_box.clip, clip.contents.clip);
  }

  #[test]
  fn absolute_children_take_their_containing_block_clip() {
    let outside = scene(div("positioned", [div("clip", [div("absolute", [])])]));
    let inside = scene(div("root", [div("positioned-clip", [div("absolute", [])])]));

    assert_eq!(properties(&outside, &[0, 0]).border_box.clip, None);
    assert_eq!(
      properties(&inside, &[0, 0]).border_box.clip,
      properties(&inside, &[0]).contents.clip
    );
  }

  #[test]
  fn effects_escaped_by_absolute_descendants_open_outside_clips() {
    let escaped = scene(div(
      "positioned",
      [div("clip", [div("faded", [div("absolute", [])])])],
    ));
    let contained = scene(div(
      "root",
      [div("clip", [div("faded", [div("child", [])])])],
    ));
    let output_clip = |scene: &Scene| {
      let effect = properties(scene, &[0, 0])
        .border_box
        .effect
        .expect("faded has an effect");

      scene.properties.effect(effect).output_clip
    };

    assert_eq!(output_clip(&escaped), None);
    assert_eq!(
      output_clip(&contained),
      properties(&contained, &[0]).contents.clip
    );
  }
}
