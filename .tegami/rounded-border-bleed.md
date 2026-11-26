---
packages:
  takumi-core: patch
---

# Rounded borders no longer show a hairline at their corners

When a box has `border-radius` and an opaque border on every side that is neither dotted nor dashed, it now paints as Chrome does:

- Its `background` stops inside the border, halfway in or a sixth of the way for a double border, so no arc of it shows along the outer edge.
- A cell of a table with `border-collapse: collapse` keeps its whole background, since it paints no border of its own.
- An outer `box-shadow` under an opaque `background-color` reaches a pixel under the border, so what sits behind the shadow no longer shows at the border's edge.
