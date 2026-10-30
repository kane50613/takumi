---
packages:
  takumi-core: patch
---

# An inline `<svg>` draws whole at a `devicePixelRatio` above one

An inline `<svg>` with a `viewBox` used to scale its shapes by the device pixel ratio twice, so at `devicePixelRatio: 2` only the top-left quarter showed. Its user units are now CSS pixels at any ratio, as Chromium draws them.
