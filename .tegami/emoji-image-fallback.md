---
packages:
  "@takumi-rs/helpers": patch
---

# A missing emoji image no longer fails the render

When the CDN cannot serve an emoji's image, such as an emoji Twemoji does not draw, the render leaves that emoji out instead of rejecting. This covers every image from an emoji CDN, so a `takumi-js` render and a renderer's own `prepare` behave the same. `images.throwOnError` still rejects when any other image fails.
