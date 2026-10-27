import { readFile } from "node:fs/promises";
import wasmUrl from "@takumi-rs/wasm/wasm-url";
import { initWasm } from "./wasm-init";
import type { LoadBackend } from "./types";

// The WebContainer fallback for the node backend. `@takumi-rs/wasm/node` would
// instantiate the binary when evaluated, so this reads it on demand instead.
export const loadBackend: LoadBackend = (module) => initWasm(module, () => readFile(wasmUrl));
