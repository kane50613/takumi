---
packages:
  takumi-core: patch
---

# Parse `animation` and run Tailwind's keyframes as Chrome and Tailwind do

The `animation` shorthand now reads values the way Chrome does.

- A unitless `0` is an iteration count, so `animation: fade 1s 0` runs nothing.
- Each longhand takes one value. A repeated keyword becomes the name, or the declaration is dropped.
- `animation-duration: auto` computes to 0, and a negative duration drops the declaration.

```css
.card {
  animation: fade 1s 0;
}
/* Before: fade runs once, with the 0 read as its delay */
/* After: fade runs zero times, as in Chrome */
```

When animations stack on one property, a keyframe an animation leaves out takes the value from the animations before it.

The keyframes behind `animate-spin`, `animate-ping`, `animate-pulse`, and `animate-bounce` now follow Tailwind's theme. `bounce` eases each half as Tailwind does, and `spin` and `ping` animate `transform`. `spin` turns through quarter turns, so it matches Tailwind only under the `linear` easing `animate-spin` uses.

These keyframes exist once an `animate-*` class, an arbitrary `animate-[…]` value, or an `@apply` names them. A plain `animation: spin 1s` without its own `@keyframes spin` stays still, as in Chrome. Your own `@keyframes` with the same name still wins.

#### Migration

`StyleDeclaration::animation_duration` takes `AnimationDuration` values instead of bare `AnimationTime` values.

```diff
- StyleDeclaration::animation_duration([AnimationTime::from_milliseconds(1000.0)].into())
+ StyleDeclaration::animation_duration([AnimationDuration(AnimationTime::from_milliseconds(1000.0))].into())
```
