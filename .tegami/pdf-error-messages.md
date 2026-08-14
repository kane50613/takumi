---
packages:
  takumi-pdf:
    type: patch
---

### Say what a failed render needs

A failed render threw the error's Rust shape, such as `MissingGlyphs("क (U+0915)")` or `DecodeError(Unsupported(UnsupportedError { format: Unknown }))`. Every error now reads as a sentence that names the fix, and the ones wrapping another error carry its message instead of its debug form.
