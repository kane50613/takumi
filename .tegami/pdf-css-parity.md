---
packages:
  takumi-pdf:
    type: minor
---

### Close the CSS paint gaps against the raster backend

`outline`, `text-shadow`, `-webkit-text-stroke`, `url()` background and mask layers, `background-origin`, `background-clip` (including `text` and `border-area`) and `background-blend-mode` now paint.

- outlines ride the border machinery: offset outward, following the radius, no layout impact
- text shadows draw as shifted glyph passes under the text; PDF has no blur operator, so a blurred one draws sharp
- `background-clip: text` fills the glyphs with the background color and gradient layers, so gradient text stays selectable vector text
- url() layers rasterize like a filtered image and honor intrinsic sizing, so `background-size: auto`, `cover` and `contain` resolve like the raster backend
