---
"@takumi-rs/helpers": patch
---

Use a per-request timeout in `fetchResources` so one slow URL no longer consumes the whole batch's timeout budget
