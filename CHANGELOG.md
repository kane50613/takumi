## takumi

### Place an inset `box-shadow` in the padding box

An inset `box-shadow` was placed against the border box instead of the padding box. On a box with a border the shadow fell at the border's position, where the opaque border covered it. The shadow is now placed against the padding box and appears just inside the border.

## takumi

### Restore JPEG, WebP and GIF decoding

`takumi` disables `takumi-core`'s default features and never re-enables image decoding, so every build decoded PNG and ICO only. A WebP source failed with `The image format could not be determined`. `image-decoding` is now a `takumi` feature as well, on by default, and it splits into `jpeg`, `webp` and `gif` for a build that wants one format and not the others. The napi and wasm bindings turn it on too.
