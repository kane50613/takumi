---
packages:
  "takumi-core":
    type: patch
---

### Size corner-keyword radial gradients like browsers

An ellipse `radial-gradient` sized `farthest-corner` (the default) or
`closest-corner` now passes through that corner while keeping the matching
side keyword's aspect ratio. Both sizes previously stopped at the box sides,
so the default radial gradient rendered smaller than in browsers with a hard
edge at its boundary.
