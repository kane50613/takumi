---
packages:
  npm:takumi-js:
    type: minor
---

### Add `ImageResponse.render` for a fully rendered response

`ImageResponse.render(element, options)` awaits the render and returns a
`Response` carrying the finished bytes instead of a stream, so the runtime knows
the body length and the headers carry a strong `ETag` of the image. The
constructor cannot set one, since its headers are read while the render is still
in flight.
