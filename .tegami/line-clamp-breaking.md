---
packages:
  takumi-core: minor
---

# `line-clamp` on a flex or grid container no longer clamps, and `max-lines` is a `MaxLines`

`display: flex; line-clamp: 2` used to clamp the text inside the container. Chrome clamps only a block container, so this no longer clamps, and neither does `display: grid`. Put `line-clamp` on a block inside the flex or grid container instead.

The `max-lines` longhand is now a `MaxLines` in place of an `Option<u32>`, so it can hold `auto` beside a count. `StyleDeclaration::max_lines` takes one, and `StyleDeclaration::MaxLines` and `LineClamp::max_lines` hold one. `MaxLines` converts from a `u32` or an `Option<u32>`, where `None` is `auto`.
