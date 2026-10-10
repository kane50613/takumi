---
packages:
  "takumi": patch
---

# Encode PNG faster

PNG data is deflated with a shorter match search, so encoding takes 10% less time and 7% less CPU across real templates, for files 0.13% larger.
