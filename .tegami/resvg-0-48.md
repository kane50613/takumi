---
packages:
  takumi-core:
    type: patch
---

### Vendored resvg updated to 0.48.1

Pulls the upstream parser and filter fixes: nested `svg` transforms are no longer applied twice, a missing `width`/`height` is computed from the viewBox aspect ratio, `href` takes precedence over `xlink:href`, `fr` is inherited for radial gradients referenced via `href`, and oversized filter regions no longer panic.
