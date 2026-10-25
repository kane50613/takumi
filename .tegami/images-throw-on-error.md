---
packages:
  takumi-js: minor
---

# Render without the images that fail to fetch

The `images` option of `render()` and `ImageResponse` now takes `throwOnError`. Set it to `false` and an image that fails to fetch, such as an emoji Twemoji has no image for, is left out instead of rejecting the whole render. It defaults to `true`, so existing renders behave as before.
