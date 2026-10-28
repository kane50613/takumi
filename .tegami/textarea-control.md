---
packages:
  takumi-core: minor
  takumi-html: minor
---

# Lay out `<textarea>` as a field sized by `cols` and `rows`

A `<textarea>` now takes Chromium's default monospace font, border, and padding, and sizes itself from `cols` and `rows` with room for a scrollbar. Its text keeps its line breaks and, unless the field is disabled or read-only, its spaces. It wraps inside the field, which clips it in both axes even under `overflow: visible`, as Chromium does. A disabled textarea turns gray. A textarea used to render as inline text with its line breaks collapsed.
