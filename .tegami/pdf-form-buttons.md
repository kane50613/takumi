---
packages:
  takumi-pdf:
    type: minor
---

### Render check boxes and radio groups as PDF buttons

`form: true` emits `<input type="checkbox">` as a check box and every
`<input type="radio">` sharing a `name` as one radio group. The group holds each
button as a kid, writes the checked one to `/V` and `/DV`, and keeps the values
they submit in `/Opt`, so an export value a PDF name cannot carry still survives.
