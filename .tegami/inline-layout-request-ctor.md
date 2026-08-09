---
packages:
  takumi-core:
    type: minor
---

### Build an inline layout request from the box it fills

`InlineLayoutRequest::in_content_box` takes the content box and works out the available space, the wrap width and the height clamp from it. `in_available_space` does the same from taffy's constraint, for a box still sizing itself. Every backend spelled all three out by hand at seven call sites.

`create_inline_constraint` and `resolve_inline_max_height` no longer leave the crate; the constructors call them.
