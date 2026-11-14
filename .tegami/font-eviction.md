---
packages:
  takumi-core:
    type: minor
  takumi:
    type: minor
  "@takumi-rs/helpers":
    type: patch
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  takumi-pdf:
    type: minor
  takumi-paint:
    type: minor
---

# Evict fonts one file at a time from the renderer's cache

Font files that renders load now share the renderer's `maxBytes` pool and leave one file at a time, least recently used first, so a family that keeps gaining files, such as one Google Fonts `text=` subset per title, stays within the budget. A render keeps the fonts it uses until it finishes, the same bytes count once under any key, and `registerFont` fonts still stay. Among equally matching faces of one family, the one listed last wins every time.
