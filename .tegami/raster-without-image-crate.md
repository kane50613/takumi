---
packages:
  takumi-raster: patch
---

# Write ICO files without the `image` crate

`OutputFormat::Ico` now wraps the image in an ICO container around the same PNG `write_image` produces, so the icon gets its palette and filter choices and `takumi-raster` no longer depends on `image`. The error for an icon over 256 pixels on a side reads differently.
