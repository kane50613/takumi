---
packages:
  "takumi": patch
---

# Blur filters like Chrome

`filter: blur()`, `backdrop-filter` and `text-shadow` now blur with the Gaussian Chrome's software renderer uses for them: a true Gaussian kernel for small blurs and three box passes for larger ones. A large blur keeps full resolution instead of being blurred at a reduced size and scaled back up, so it matches Chrome and renders faster.
