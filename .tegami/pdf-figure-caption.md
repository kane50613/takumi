---
packages:
  takumi-pdf:
    type: patch
---

### Tag `<figure>` as a Figure

A `<figure>` now becomes a `Figure` structure element carrying its image's `alt`, and the `<figcaption>` inside it becomes a `Caption` child of that element. The caption used to float up to the document root, where no standard allows it.
