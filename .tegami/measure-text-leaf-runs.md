---
packages:
  "@takumi-rs/core":
    type: patch
---

### Report text runs for a node that holds text directly

`measure` returned no text runs for `<div>word</div>`, the shape an HTML parse
produces most often, while `<div><span>word</span></div>` reported them. The
render was correct either way; only the measurement was missing its geometry.
