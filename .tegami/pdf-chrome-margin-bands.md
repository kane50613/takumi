---
packages:
  takumi-pdf:
    type: minor
---

### Draw header and footer bands in the page margins

Bands previously reserved their height inside the content window, so a footered document paginated earlier than Chrome's print output. They now lay out at full page width and draw in the margin areas with Chromium's 15pt edge inset. The content window always spans the full margin box.
