---
packages:
  takumi-pdf:
    type: minor
---

### Reject a page that would print wrong

An image whose bytes will not decode used to leave a hole, and `filter: blur()` or `drop-shadow()` used to be dropped without a word. Both now stop the render and name what went wrong, the way an uncovered character already did.
