---
packages:
  takumi-core: patch
---

# Blurs and shadows render faster

These effects render faster, and their pixels can change after upgrading:

- `filter: blur()` and `drop-shadow()`
- `backdrop-filter`
- `box-shadow` and `text-shadow`
- SVG `feGaussianBlur`

Review the diffs if you compare renders against pixel-exact snapshots.
