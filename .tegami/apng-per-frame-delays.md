---
packages:
  "cargo:takumi-raster": patch
---

### Write per-frame delays into animated PNG output

Every APNG frame was written with the shortest frame's delay. The delay was set once on the encoder, before the header, so the header's `fcTL` covered the whole animation, on the premise that APNG has no per-frame duration. It does: each frame carries its own `fcTL`, and the `png` crate exposes it as `Writer::set_frame_delay`.

The header now takes the first frame's duration and every later frame gets its own. A 150 ms timeline that renders as frames of 33, 33, 34, 33 and 17 ms used to play back as five 17 ms frames, roughly 1.8× too fast. A short scene followed by a long hold was much worse. WebP and GIF already wrote per-frame delays and are unchanged.
