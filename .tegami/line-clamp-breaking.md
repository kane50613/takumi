---
packages:
  takumi-core: minor
---

# `line-clamp` on a flex or grid container no longer clamps, `max-lines` is a `MaxLines`, and `max-lines: none` is invalid

`display: flex; line-clamp: 2` used to clamp the text inside the container. Chrome clamps only a block container, so this no longer clamps, and neither does `display: grid`. Put `line-clamp` on a block inside the flex or grid container instead.

The `max-lines` longhand is now a `MaxLines` in place of an `Option<u32>`, so it can hold `auto` beside a count. `StyleDeclaration::max_lines` takes one, and `StyleDeclaration::MaxLines` and `LineClamp::max_lines` hold one. Read it through `MaxLines::lines` and `MaxLines::has_auto`, and build it with `MaxLines::new` or from a `u32` or an `Option<u32>`, where `None` or a zero count is `auto`. `max-lines: none` is now invalid, as in Chrome, so a later `max-lines: none` no longer overrides an earlier `line-clamp: 3`.
