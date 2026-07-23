---
packages:
  "@takumi-rs/core": patch
---

### Stop copying Uint8Array font and image inputs on the native binding

Buffer inputs were already passed to render tasks as ref-counted views, but Uint8Array inputs went through a full `to_vec` copy first. Both now cross into the async tasks zero-copy; only bare ArrayBuffer inputs still copy.
