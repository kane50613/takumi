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

Font files that renders load now share the renderer's `maxBytes` pool, and leave it one file at a time, least recently used first. A family that keeps gaining files, such as one Google Fonts `text=` subset per title, now stays within the budget.

- A render keeps the fonts it uses until it finishes.
- The same bytes count once, whatever key loaded them.
- Fonts from `registerFont` still stay.
- Among equally matching faces of one family, the one listed last now wins every time.
