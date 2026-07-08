---
packages:
  cargo:takumi:
    replay:
      - exit-prerelease(cargo:takumi)
---

### Fix `fontFamilies` order being ignored

`fontFamilies` only fed the fallback bucket, never the root style, so text
picked whichever registered font resolved first instead of the requested
order. `FontFamily`'s default is now empty instead of a generic `sans-serif`
token, so an empty root style falls through to the fallback bucket directly.
