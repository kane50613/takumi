---
packages:
  npm:@takumi-rs/core:
    replay:
      - exit-prerelease(npm:@takumi-rs/core)
  npm:@takumi-rs/wasm:
    replay:
      - exit-prerelease(npm:@takumi-rs/wasm)
---

### Type keyframe declarations with `csstype` instead of DOM's `CSSStyleDeclaration`

`KeyframesMap` and `KeyframeRule` typed each keyframe's declarations as
`Record<string, CSSStyleDeclaration>`, requiring every CSS property on a single
offset and needing the `DOM` lib. Declarations are now typed with `csstype`'s
`Properties`, an optional peer dependency, so a single offset only needs the
properties it sets and consumers without the `DOM` lib still typecheck.
