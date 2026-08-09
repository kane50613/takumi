---
packages:
  takumi-core:
    type: minor
  takumi-raster:
    type: patch
  takumi-svg:
    type: patch
  takumi-pdf:
    type: patch
---

### Stroke the span that asked for it

`-webkit-text-stroke` was read off the element holding the text, so a `span` setting it for itself came out unstroked, and a nested one turning it off still got the parent's outline. The stroke now travels with the text run, in every backend.
