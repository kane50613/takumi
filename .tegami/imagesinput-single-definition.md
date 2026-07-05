---
packages:
  npm:takumi-js:
    type: major
---

### Give `ImagesInput` one definition

`takumi-js` and `@takumi-rs/helpers` each exported a different type named
`ImagesInput`. The base shape now lives only in `@takumi-rs/helpers` and
`takumi-js` re-exports it, so `ImagesInput` means the same thing everywhere. The
fetch-aware superset that `render`/`renderSvg`/`renderAnimation` accept is now
`ManagedImagesInput`.
