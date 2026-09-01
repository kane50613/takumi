---
packages:
  "takumi-core":
    type: patch
---

### Size corner-keyword radial gradients like browsers

An ellipse `radial-gradient` sized `farthest-corner` (the default) or
`closest-corner` passes through that corner, as browsers do; both keywords
previously stopped at the box sides. Side distances are measured absolute,
so a center outside the box keeps positive radii.
