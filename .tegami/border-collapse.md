---
packages:
  "@takumi-rs/core":
    type: minor
---

### Collapse a table's borders onto shared lines

`border-collapse: collapse` did nothing, so neighbouring cells each painted their own border and the pair of lines sat either side of the `border-spacing` gap. A collapsing table now resolves every grid line once: the wider border wins, `hidden` clears the line, and a row's borders reach its cells the way its background already did. The winner is drawn whole inside the cell below or right of the line rather than straddling it, so the table's outer edge sits half a border width inside where Chrome puts it.
