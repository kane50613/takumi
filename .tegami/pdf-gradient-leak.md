---
packages:
  takumi-pdf:
    type: patch
---

# A gradient with more than two color stops no longer leaks memory

Every PDF that drew a gradient with three or more stops, such as `conic-gradient(red, yellow, green, blue)`, kept part of that gradient's function in memory after the render finished. A long-running process that renders many such PDFs now holds steady.
