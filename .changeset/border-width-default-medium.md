---
"takumi-css": patch
"takumi": patch
---

Default an omitted `border-width` / `outline-width` to `medium` (~3px) instead of `0`, so `border: solid red` and `outline: solid` paint a visible line as CSS specifies. Borders with no visible style (`border: none`, color-only, empty) keep width `0`.
