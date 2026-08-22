---
packages:
  "@takumi-rs/core":
    type: minor
---

### Animate WebP image sources

An animated WebP used as an image source now plays through its frames, the same way an animated GIF does. Before, decoding one threw `WebP encode failed` and took the whole render down with it. The still-image paths, which the SVG backend and the size cache go through, fall back to the first frame instead of failing.

`ImageSource::Gif` is now `ImageSource::Animated`, and `GifSource` is now `AnimatedSource`, since both carry GIF and WebP animations. `ImageError::InvalidGif` is now `ImageError::InvalidAnimation` for the same reason.

An animated source keeps only its encoded bytes and a frame timing table. It used to hold a decoded full-size copy of the first frame as well, which also meant the first frame ignored the draw box it was scaled into.
