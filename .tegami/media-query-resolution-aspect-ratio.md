---
packages:
  "takumi-core":
    type: minor
---

### Match `resolution` and `aspect-ratio` media queries

`@media (resolution >= 2dppx)` reads the render's device pixel ratio, in
`dppx`, `x`, `dpi`, or `dpcm`. `@media (aspect-ratio >= 16/9)` reads the
viewport's ratio. Both take the range syntax and the `min-`/`max-` prefixes.
