---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Keep Google Fonts cache entries within caller limits

Bound the default `googleFonts` CSS cache, isolate custom fetch implementations, and honor request policies on repeated calls.
