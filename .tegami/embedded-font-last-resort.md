---
packages:
  cargo:takumi:
    replay:
      - exit-prerelease(cargo:takumi)
  npm:@takumi-rs/core:
    replay:
      - exit-prerelease(npm:@takumi-rs/core)
  npm:@takumi-rs/wasm:
    replay:
      - exit-prerelease(npm:@takumi-rs/wasm)
---

### Make the embedded font a true last resort

Both bindings now embed one font: a Latin Geist subset with a 400 to 700
weight axis (Geist Mono and Manrope are gone). It no longer claims the
`sans-serif` generic family and always sorts after registered fonts in
fallback selection, so generic families and unstyled text resolve to the fonts
you load. The new `FontResource::last_resort` marks a font's families to sort
after every normal registration.
