---
packages:
  takumi-core: patch
---

# A forced break inside a nested flex row moves the row

`break-before: page` on the first item of a flex container nested in a flex row used to break only inside the nested container, so its first row stayed on the page before. The break now reaches the row around it, and the whole nested container starts the next page, as in Chromium.
