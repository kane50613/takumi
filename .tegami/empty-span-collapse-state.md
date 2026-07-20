---
packages:
  "cargo:takumi-core": patch
---

### Keep whitespace collapse state across empty inline spans

An empty span with `white-space: pre` reset the cross-span collapse state, so a boundary space next to it could double up or vanish. Empty spans now leave the state untouched, matching Blink's opaque-to-collapsing empty text items.
