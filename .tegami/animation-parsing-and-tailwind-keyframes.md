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

The keyframes behind `animate-spin`, `animate-ping`, `animate-pulse`, and `animate-bounce` now follow Tailwind's theme. `bounce` eases each half as Tailwind does, and `spin` and `ping` animate `transform`.

These keyframes exist once an `animate-*` class, an arbitrary `animate-[…]` value, or an `@apply` in the render names them, directly or through the `--animate-*` theme value it reads and the `var(--animate-*)` values that one refers to, and then any `animation` in the render can use them. So `animate-soft` with `@theme { --animate-soft: pulse 2s }` runs `pulse`. A plain `animation: spin 1s` stays still, as in Chrome, when nothing in the render names `spin` and the stylesheet has no `@keyframes spin`. Your own `@keyframes` with the same name still wins.

#### Migration

`animation-duration` and `animation-delay` need a unit, so Chrome and takumi now drop a declaration with a unitless `0`. Write `0s` instead. In the `animation` shorthand, a unitless `0` now sets the iteration count, so write `0s` to keep it as the delay.

```diff
- animation-duration: 0;
- animation-delay: 0;
- animation: fade 1s 0;
+ animation-duration: 0s;
+ animation-delay: 0s;
+ animation: fade 1s 0s;
```

`StyleDeclaration::animation_duration` takes `AnimationDuration` values instead of bare `AnimationTime` values.

```diff
- StyleDeclaration::animation_duration([AnimationTime::from_milliseconds(1000.0)].into())
+ StyleDeclaration::animation_duration([AnimationDuration(AnimationTime::from_milliseconds(1000.0))].into())
```
