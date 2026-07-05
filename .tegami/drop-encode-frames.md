---
packages:
  npm:@takumi-rs/core:
    replay:
      - exit-prerelease(npm:@takumi-rs/core)
  npm:@takumi-rs/wasm:
    replay:
      - exit-prerelease(npm:@takumi-rs/wasm)
  npm:takumi-js:
    replay:
      - exit-prerelease(npm:takumi-js)
---

### Remove `encodeFrames`

`Renderer.encodeFrames` and its `EncodeFramesOptions` / `AnimationFrameSource`
types are gone. `renderAnimation` covers scene-based animation; pre-rendered
frame encoding had no callers.
