# takumi-raster

<!-- cargo-rdme start -->

Raster painting backend for takumi, built on ports of Chromium's Skia scan converters and shaders.

[`render`] paints a node tree into a [`Bitmap`], and [`write_image`] encodes it as PNG,
JPEG, WebP, or ICO. [`write_animation`] encodes animated WebP, APNG, or GIF.

Use it through the `takumi` crate, which re-exports these functions at its root. The whole
crate is `takumi::unstable::raster`. This crate does not re-export core types; reach them
through `takumi_core` or `takumi::unstable::base`.

<!-- cargo-rdme end -->
