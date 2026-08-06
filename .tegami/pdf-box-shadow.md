---
packages:
  takumi-pdf:
    type: minor
---

### Draw `box-shadow`

Outer and inset shadows now paint. The offset, spread and rounded corners are exact: the shadow is the border box spread and moved, with the box itself cut out by an even-odd fill so nothing paints under an opaque element.

PDF has no blur operator, so a blurred shadow is approximated by eight bands whose opacity follows the coverage the blur would leave. A shadow with no blur draws as one exact fill.
