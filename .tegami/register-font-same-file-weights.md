---
packages:
  takumi-core: patch
---

# A font file registered at two weights keeps a face for each

Registering one file under the same family with another `weight`, `style`, width or `unicodeRange` used to reuse the first face, so `font-weight: 700` drew a synthetic bold of the face declared at 400. Each registration with its own descriptors now adds its own face, sharing the parsed font.
