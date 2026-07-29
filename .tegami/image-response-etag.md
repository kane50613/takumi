---
packages:
  npm:takumi-js:
    type: minor
---

### Add `imageResponse` for an `ETag`-carrying response

`imageResponse(element, options)` awaits the render and returns a `Response` whose
body is the image bytes and whose `ETag` is their SHA-256 digest. `new
ImageResponse(...)` cannot set one, since its headers are read while the render is
still in flight.
