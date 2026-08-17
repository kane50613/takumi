---
packages:
  "@takumi-rs/core":
    type: minor
---

### Share custom property maps across nodes

`ComputedStyle::custom_properties` and `registered_custom_properties` are now `Arc`-shared, so inheriting them no longer copies the maps per node.
