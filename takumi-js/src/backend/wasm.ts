import autoModule from "@takumi-rs/wasm/auto";
import { initWasm } from "./wasm-init";
import type { LoadBackend } from "./types";

// Every non-Node condition lands here, and `@takumi-rs/wasm/auto` picks the binary loader.
export const loadBackend: LoadBackend = (module) => initWasm(module, autoModule);
