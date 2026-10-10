---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Register WOFF2 fonts faster

Registering a WOFF2 font takes about 25% less time, so a cold start with a large CJK font is ready sooner.
