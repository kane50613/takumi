import autoModule from "@takumi-rs/wasm/auto";
import { initWasm } from "./wasm-init";
import type { LoadBackend } from "./types";

// Selected by every non-Node condition (workerd, worker, deno, browser, and the
// default fallback). `@takumi-rs/wasm/auto` resolves to the right binary loader
// for the host bundler via its own conditions.
export const loadBackend: LoadBackend = (module) => initWasm(module, autoModule);
