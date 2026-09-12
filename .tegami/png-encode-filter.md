---
"takumi": patch
---

# Encode photographic PNG output faster and smaller

PNG output now picks its encoder settings from the image. Photo backgrounds encode about twice as fast and around 30% smaller. Flat art keeps its bytes. Animated PNG picks its settings from the first frame.
