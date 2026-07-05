---
packages:
  npm:@takumi-rs/helpers:
    replay:
      - exit-prerelease(npm:@takumi-rs/helpers)
---

### Shrink the generated Google Fonts type catalog

Group the ~1940 families by their distinct weight/style/axis shape (152 of them)
so `GoogleFontFamily` builds a discriminated union over shapes, not families. The
resolved types are unchanged, so autocomplete is identical, but the shipped `.d.ts`
drops from ~192 KB to ~58 KB and the checker does ~75% fewer instantiations on the
type. The generator now refuses to write a catalog with under 1000 families.
