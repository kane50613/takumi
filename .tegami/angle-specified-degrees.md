---
packages:
  takumi-core: patch
---

# Rotations past a full turn animate

An angle now keeps the degrees you write, as in Chrome, so animations interpolate the number itself instead of the shorter way round. `rotate(360deg)` turns once and `rotate(720deg)` turns twice, where both used to stand still. The same applies to `rotate`, `skew()`, `hue-rotate()`, and `offset-rotate`.

`animate-spin` now uses Tailwind's own keyframes, a single `to { transform: rotate(360deg) }`.

`font-style: oblique 350deg` is now invalid, as in Chrome, instead of reading as `-10deg`.

```css
@keyframes turn {
  to {
    transform: rotate(360deg);
  }
}
/* Before: no rotation at any point */
/* After: 90deg a quarter of the way through */
```
