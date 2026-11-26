---
packages:
  takumi-core: patch
---

# Tailwind `max-*` and `min-*` breakpoints, and stacked breakpoints

`max-md:`, `min-[600px]:`, and `max-[800px]:` now compare the viewport width as Tailwind does, and `sm:max-md:` applies only while both hold.
