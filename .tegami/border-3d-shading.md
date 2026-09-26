---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Shade `inset`, `outset`, `groove`, and `ridge` borders the way Chrome does

These styles used to mix the border color a third of the way toward white or black. They now shade it the way Chrome does: the shadowed edges darken, the lit edges keep the color unless there is too little contrast, and a very dark color lightens instead so both edges stay visible.
