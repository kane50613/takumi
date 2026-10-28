---
packages:
  takumi-core: minor
---

# Match form control states such as `:disabled` and `:checked`

`:disabled`, `:enabled`, `:checked`, `:indeterminate`, `:placeholder-shown`, `:read-only`, `:read-write`, `:required`, and `:optional` now match by the element's attributes, as HTML defines them. A control inside `<fieldset disabled>` counts as disabled, and an `<option>` counts as checked when its `<select>` would show it selected. These selectors used to match nothing, so their rules never applied.
