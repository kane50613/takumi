---
packages:
  takumi-pdf:
    type: patch
---

### Keep clipped-away content off every page

Content an `overflow` clip cut away still reached the file when it sat far enough down the page to land on a later one. A clip keeps it off the page, but not out of the text layer, so a redacted or collapsed section came back out of any tool that reads text: search, copy, an accessibility reader.
