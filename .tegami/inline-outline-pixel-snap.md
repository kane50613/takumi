---
packages:
  "takumi": patch
---

# Snap inline outlines to the pixels Chrome snaps them to

- An inline element's outline snaps its width and height from 1/64px layout units, as Chrome does, so it no longer ends up 1px narrower than Chrome's.
