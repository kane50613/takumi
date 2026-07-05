---
packages:
  "cargo:takumi-raster": minor
  "cargo:takumi": minor
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
---

### Stream animation frames straight into the encoder

Add `write_animation`, which renders a timeline and feeds each frame to the
encoder as it arrives, holding one raw frame at a time instead of the whole
sequence. Both the napi and WASM `renderAnimation` bindings use it, so a high
frame rate or a long duration no longer exhausts memory. On native the WebP
encoder still runs frames in parallel, now over bounded chunks. The WASM WebP
encoder now merges runs of identical frames like the native one, so a static or
slow animation encodes and stores far less. The eager `render_animation` +
`write_animated_*` path stays for callers that want every frame at once.
