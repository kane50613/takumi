---
packages:
  takumi-pdf:
    type: minor
---

### Render a select as a fillable drop-down

`form: true` emits `<select>` as an AcroForm choice field. `/Opt` pairs each
option's export value with its label, which a non-empty `label` attribute
outranks. `/V` and `/DV` hold what the selected options submit, following
HTML's selectedness rules. `multiple` or a `size` above one becomes a list box
drawing every option as a row.
