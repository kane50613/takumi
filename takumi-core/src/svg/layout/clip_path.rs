use std::sync::Arc;

use super::{IdKind, State, convert_clip_path_elements};
use crate::{
  resvg::usvg::{ClipPath, Group, NonEmptyString, NonZeroRect, Transform, Units},
  svg::{Attr, Node as SvgNode, Tag},
};

pub(crate) fn convert(
  node: SvgNode,
  state: &State,
  object_bbox: Option<NonZeroRect>,
  cache: &mut super::Cache,
) -> Option<Arc<ClipPath>> {
  // A `clip-path` attribute must reference a `clipPath` element.
  if node.tag() != Some(Tag::ClipPath) {
    return None;
  }

  // The whole clip path should be ignored when a transform is invalid.
  let mut transform = resolve_clip_path_transform(node, state)?;

  let units = node
    .parse(Attr::ClipPathUnits)
    .unwrap_or(Units::UserSpaceOnUse);

  // Check if this element was already converted.
  //
  // Only `userSpaceOnUse` clipPaths can be shared,
  // because `objectBoundingBox` one will be converted into user one
  // and will become node-specific.
  let cacheable = units == Units::UserSpaceOnUse;
  if cacheable && let Some(clip) = cache.clip_paths.get(node.element_id()) {
    return Some(clip.clone());
  }

  if units == Units::ObjectBoundingBox {
    let object_bbox = match object_bbox {
      Some(v) => v,
      None => {
        log::warn!("Clipping of zero-sized shapes is not allowed.");
        return None;
      }
    };

    let ts = Transform::from_bbox(object_bbox);
    transform = transform.pre_concat(ts);
  }

  // Resolve linked clip path.
  let mut clip_path = None;
  if let Some(link) = node.reference(&node.style()?.clip_path) {
    clip_path = convert(link, state, object_bbox, cache);

    // Linked `clipPath` must be valid.
    clip_path.as_ref()?;
  }

  let mut id = NonEmptyString::new(node.element_id().to_string())?;
  // Generate ID only when we're parsing `objectBoundingBox` clip for the second time.
  if !cacheable && cache.clip_paths.contains_key(id.get()) {
    id = cache.gen_id(IdKind::ClipPath);
  }
  let id_copy = id.get().to_string();

  let mut clip = ClipPath {
    id,
    transform,
    clip_path,
    root: Group::empty(),
  };

  let mut clip_state = state.clone();
  clip_state.parent_clip_path = Some(node);
  convert_clip_path_elements(node, &clip_state, cache, &mut clip.root);

  if clip.root.has_children() {
    clip.root.calculate_bounding_boxes();
    let clip = Arc::new(clip);
    cache.clip_paths.insert(id_copy, clip.clone());
    Some(clip)
  } else {
    // A clip path without children is invalid.
    None
  }
}

fn resolve_clip_path_transform(node: SvgNode, state: &State) -> Option<Transform> {
  // A singular transform hides the whole clip path.
  match node.style()?.transform {
    Some(transform) if !transform.is_valid() => None,
    _ => Some(node.resolve_transform(state)),
  }
}
