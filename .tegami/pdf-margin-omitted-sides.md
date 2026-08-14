---
packages:
  takumi-pdf:
    type: minor
---

### Default an omitted margin side to `auto`

A side left out of the `margin` object used to sit flush with the paper edge, which put a band on that side straight over the content. It now defaults to `"auto"`, the same as every other side. Pass `0` to get the old behaviour.
