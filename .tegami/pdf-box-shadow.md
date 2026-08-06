---
packages:
  takumi-pdf:
    type: minor
---

### Draw `box-shadow`

Outer and inset shadows now paint. The offset, spread and rounded corners are exact: the shadow is the border box spread and moved, with the box itself cut out by an even-odd fill so nothing paints under an opaque element.

PDF has no blur operator, so a blurred shadow is approximated by eight bands whose opacity follows the Gaussian edge coverage CSS specifies, with a standard deviation of half the blur radius. A shadow with no blur draws as one exact fill.

Inset shadows draw inside the padding box, so a border neither carries shadow paint nor widens the shadow.
