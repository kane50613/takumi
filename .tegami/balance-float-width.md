---
packages:
  "takumi": patch
---

# Balance text beside floats against the container

`text-wrap: balance` searched for its width with the floats squeezed into that narrower width, so paragraphs beside a float ended up barely balanced.
