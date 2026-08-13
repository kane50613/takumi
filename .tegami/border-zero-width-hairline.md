---
packages:
  takumi-core:
    type: patch
---

### Stop drawing a hairline on borderless sides

A box with a border on only some sides filled one even-odd ring, which left the inner and outer contours sharing every borderless edge. PDF and SVG viewers antialias each edge on its own, so those edges kept about half their coverage and printed as faint lines. Such a border now draws side by side, which carries no shared edges.
