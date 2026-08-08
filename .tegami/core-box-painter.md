---
packages:
  takumi-core:
    type: minor
---

### Decide a box's paint in one place

`BoxPainter` answers what a box paints, so a backend asks it instead of working the same things out again. It covers `background-color`, the `background-clip` shape, and `outline`.

An outline with no width, or one whose style draws nothing, now paints nothing in every backend. One of them used to skip both checks.
