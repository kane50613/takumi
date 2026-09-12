---
"takumi": patch
---

# Encode photographic PNG output faster and smaller

PNG output now samples the image and picks between an unfiltered level 7 deflate for flat art and an adaptive-filtered level 3 deflate for photographic content. Renders with photo backgrounds encode about twice as fast and around 30% smaller. Flat renders and animated PNG stay byte-identical.
