---
packages:
  cargo:takumi-raster:
    replay:
      - exit-prerelease(cargo:takumi-raster)
  cargo:takumi:
    replay:
      - exit-prerelease(cargo:takumi)
  npm:@takumi-rs/wasm:
    replay:
      - exit-prerelease(npm:@takumi-rs/wasm)
---

### Stream animation frames straight into the encoder

Add `write_animation`, which renders a timeline and feeds each frame to the
encoder as it arrives, holding one raw frame at a time instead of the whole
sequence. The WASM `renderAnimation` binding uses it, so a high frame rate or a
long duration no longer exhausts memory. The eager `render_animation` +
`write_animated_*` path stays for callers that want every frame at once.
