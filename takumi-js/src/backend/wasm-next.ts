import nextModule from "@takumi-rs/wasm/next";
import { initWasm } from "./wasm-init";
import type { LoadBackend } from "./types";

// Selected by the `edge-light` condition (Next.js / Vercel Edge). `/next` ships
// the `?module` binary import those runtimes need, which `/auto` doesn't cover.
export const loadBackend: LoadBackend = (module) => initWasm(nextModule, module);
