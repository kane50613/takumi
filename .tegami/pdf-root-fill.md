---
packages:
  "cargo:takumi-pdf": patch
---

### Fill the page content box like a browser body

A fit-content root resolved child percentage widths inconsistently across layout passes. Long documents could overlap trailing content and drop pages entirely.
