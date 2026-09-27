---
packages:
  "takumi": patch
---

# Clamp `grayscale()` and `sepia()` at 100% in images

`grayscale()` and `sepia()` above 100% now act as 100%, as the Filter Effects spec and the other output formats do. The image output used to push the colors past full gray or sepia. These filters also round each channel to the nearest value instead of down.
