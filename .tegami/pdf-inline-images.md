---
packages:
  takumi-pdf:
    type: patch
---

### Draw inline images and containers

An image drew nothing unless it was the root's direct child. A `<div>`, a `<figure>`, or a plain container node was enough to lose it. Wrapped images now paint, and reach the structure tree as a `Figure`.

`display: inline-block`, `inline-flex`, `inline-grid`, and `float` painted nothing at all, including their text and images. They now lay out and paint at the position the surrounding line gives them.
