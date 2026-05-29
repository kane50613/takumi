---
"takumi-js": patch
---

Honor `AbortSignal` on the WASM render path; an already-aborted request now throws instead of rendering. Detect Deno (incl. Supabase Edge Functions) and route to the WASM bindings instead of failing to import the native addon.
