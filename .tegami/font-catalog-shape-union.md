---
packages:
  npm:@takumi-rs/helpers: minor
---

### Shrink and sharpen the Google Fonts family type

Group the ~1940 families by their distinct weight/style/axis shape (152 of them) so
`GoogleFontFamily` builds a discriminated union over shapes, not families. The shipped
`.d.ts` drops from ~192 KB to ~58 KB and the checker does ~75% fewer instantiations. The
object form now autocompletes and validates each known family's weight, style, and axes;
a bare string still accepts any name at weight 400. The generator refuses to write a
catalog with under 1000 families.
