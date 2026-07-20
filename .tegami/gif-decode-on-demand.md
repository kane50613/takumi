---
packages:
  "cargo:takumi-core": patch
---

### Decode animated GIF frames on demand instead of holding the whole timeline

Once a render scrubbed past the first frame, an animated GIF decoded and kept every remaining frame, so the encoded bytes, the first frame, and all later frames stayed resident at once — and none of it counted against the image cache budget. Frames past the first now decode at draw size when they are sampled and drop right after, so a GIF holds only its encoded bytes and first frame. Output is unchanged.
