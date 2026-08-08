---
packages:
  takumi-pdf:
    type: patch
---

### Draw images inside a wrapper

An image drew nothing unless it was the root's direct child. A `<div>`, a `<figure>`, or a plain container node was enough to lose it. Wrapped images now paint, and reach the structure tree as a `Figure`.
