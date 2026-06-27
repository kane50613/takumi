---
packages:
  "npm:@takumi-rs/core": minor
  "npm:@takumi-rs/wasm": minor
  "npm:takumi-js": minor
---

### Remove `encodeFrames`

`Renderer.encodeFrames` and its `EncodeFramesOptions` / `AnimationFrameSource`
types are gone. `renderAnimation` covers scene-based animation; pre-rendered
frame encoding had no callers.
