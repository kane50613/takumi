---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Parse inline styles faster

A CSS value that fails one way of parsing no longer builds an error message before trying the next. Building nodes from HTML with inline styles takes about 40% less time.
