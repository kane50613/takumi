---
"takumi": patch
---

Fix `-webkit-text-stroke-width` scaling with `text-fit`. Stroke width is now applied in CSS pixels regardless of the text-fit scale factor, matching Chrome's `kFontSize` text-fit method (`third_party/blink/renderer/core/style/fit_text.cc`).
