---
packages:
  "takumi": minor
---

# ⚠️ Reach custom properties through one type on `ComputedStyle`

`ComputedStyle::custom_properties` and `ComputedStyle::registered_custom_properties` are now one `custom_properties: CustomProperties` field. Read a value with `style.custom_properties.get(name)`, which returns `Option<&str>`. Constructing a `ComputedStyle` with `..Default::default()` is unaffected.
