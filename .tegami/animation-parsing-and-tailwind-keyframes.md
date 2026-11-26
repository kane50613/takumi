---
packages:
  takumi-core: patch
---

# Parse `animation` and run Tailwind's keyframes as Chrome and Tailwind do

The `animation` shorthand now reads values the way Chrome does.

- A unitless `0` is an iteration count, so `animation: fade 1s 0` runs nothing.
- Each longhand takes one value. A repeated keyword becomes the name, or the declaration is dropped.
- `animation-duration: auto` computes to 0, and a negative duration drops the declaration.

When animations stack on one property, a keyframe an animation leaves out takes the value from the animations before it.

The keyframes behind `animate-spin`, `animate-ping`, `animate-pulse`, and `animate-bounce` now match Tailwind's theme. `bounce` eases each half as Tailwind does, and `spin` and `ping` animate `transform`. These keyframes exist only once an `animate-*` class uses them, so a plain `animation: spin 1s` without its own `@keyframes spin` stays still, as in Chrome.
