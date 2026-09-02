---
packages:
  takumi-pdf:
    type: minor
---

### Render a select as a fillable drop-down

`form: true` emits `<select>` as an AcroForm choice field. `/Opt` pairs each
option's export value with its label, `/V` and `/DV` hold what the selected
options submit, and `multiple` becomes a list box holding every selection.
