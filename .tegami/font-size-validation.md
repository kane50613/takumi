---
packages:
  takumi-core: patch
---

# `font-size` and font family serialization

`font-size` now ignores a negative length and `auto`, in `font` too, and accepts `-webkit-xxx-large`. A font family named like a generic family, a CSS-wide keyword, or `default`, or one that is not an identifier, serializes quoted as in Chrome.
