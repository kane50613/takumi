---
packages:
  "takumi-pdf":
    type: minor
---

### Select output pages with pageRanges

`pageRanges` keeps only the listed pages, like a print dialog. Each entry is
a 1-based page number or an inclusive `{ from, to }` span. Layout and page
counters still run over the whole document, so a kept page shows the numbers
it would in full output.
