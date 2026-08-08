---
packages:
  takumi-pdf:
    type: patch
---

### Tag `<figure>` as a Figure

A `<figure>` becomes a `Figure` carrying its image's `alt`. The `<figcaption>` inside becomes a `Caption` child of it. The caption used to float up to the document root, where no standard allows it.
