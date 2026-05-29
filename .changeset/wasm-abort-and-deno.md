---
"takumi-js": patch
---

Honor `AbortSignal` on the synchronous WASM render path (an already-aborted request now throws instead of rendering), and route Deno / Supabase Edge Functions to the WASM bindings instead of failing to import the native addon
