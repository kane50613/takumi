---
packages:
  "npm:takumi-js": patch
---

### Keep the native core out of edge bundles

The `@takumi-rs/core` import was reachable from edge builds, pulling its native
`.node` binding into the bundle and pushing it past the runtime size limit. The
import is now gated behind an inline `NEXT_RUNTIME !== "edge"` check so the edge
bundler drops it.
