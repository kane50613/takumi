---
packages:
  "takumi-core":
    type: minor
  "takumi-pdf":
    type: minor
---

### Apply `@media print` rules to PDF output

PDF renders now match the `print` media type, and image renders match
`screen`. `Viewport::media_target` picks which one a render resolves against.
