import wasmModule from "@takumi-rs/wasm/node";
import { initWasm } from "./wasm-init";
import type { LoadBackend } from "./types";

// The WebContainer fallback for the node backend. Pinned to the `node` entry
// rather than `@takumi-rs/wasm/auto`, whose conditions resolve against the host
// bundler: a Node bundle can land on a loader written for another host, e.g.
// Turbopack sets `module` and gets Vite's `?url` import of the binary.
export const loadBackend: LoadBackend = (module) => initWasm(module, wasmModule);
