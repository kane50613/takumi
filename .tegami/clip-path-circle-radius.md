---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Size `circle()` against the whole box

`clip-path` and `offset-path` circles now resolve a percentage radius against the box's diagonal over √2 and `closest-side` or `farthest-side` against all four sides, so `circle(45%)` on a wide box is a circle instead of an ellipse. `ellipse(50% 50%)` keeps serializing as an ellipse.
