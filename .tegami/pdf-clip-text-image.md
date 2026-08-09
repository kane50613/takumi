---
packages:
  takumi-pdf:
    type: patch
---

### Fill text clipped to its background with an image

`background-clip: text` could paint a colour or a gradient through the glyphs, but not an image. The layer was dropped, and since the idiom pairs the clip with a transparent colour, the text came out invisible. An image layer now draws into a pattern the glyphs are filled with.
