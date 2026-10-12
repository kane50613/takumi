---
packages:
  "takumi": minor
---

# ⚠️ Reach custom properties through one type on `ComputedStyle`

`custom_properties` and `registered_custom_properties` are now one `custom_properties: CustomProperties` field. Read a value with `style.custom_properties.get(name)`, which returns `Option<&str>`. `..Default::default()` still works.
