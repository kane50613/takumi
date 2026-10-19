---
packages:
  takumi-core: patch
---

# Share the font registry between renders

A render's font snapshot shares the registry instead of copying it, and SVG `<text>` builds its face store once per set of registered fonts instead of once per render.
