---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Free the layout tree's memory once layout finishes

The laid-out positions no longer keep the whole layout tree's allocation alive for the rest of the render. An 11,000-row table renders in about 110MB less memory.
