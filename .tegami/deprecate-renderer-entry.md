---
packages:
  "@takumi-rs/helpers": patch
---

# Editors flag imports from `@takumi-rs/helpers/renderer`

The entry holds glue internal to the renderer packages, which no longer import it. Its exports keep working and will be removed in v3. Import types such as `FontLoader` and `ImagesInput` from the renderer package you use.
