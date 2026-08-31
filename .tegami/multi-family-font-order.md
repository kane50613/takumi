---
packages:
  "takumi-core":
    type: patch
---

### Register multi-family font files in face order

A font file carrying several families (a ttc) registers them in face order.
Fallback selection no longer varies between renderers loading the same file.
