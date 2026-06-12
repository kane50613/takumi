---
"@takumi-rs/core": patch
---

Join rayon's worker threads on N-API teardown to fix a Windows crash (`0xC0000005`) when Node exits after rendering (#763)
