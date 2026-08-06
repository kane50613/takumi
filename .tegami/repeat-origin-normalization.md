---
packages:
  takumi-core:
    type: patch
---

### Normalize far-negative repeat origins

A `background-position` far below the painted area made the repeat walk start billions of tiles away and hang. The origin now normalizes to the phase-equivalent tile at the area edge.
