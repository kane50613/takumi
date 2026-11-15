import { init as initBindings, Renderer, setGlyphCacheMaxBytes } from "@takumi-rs/wasm";
import type { Backend, BackendModule } from "./types";

// Overlapping `init` calls each instantiate the binary, and the later one strands every renderer
// made on the earlier one, so all callers share the first.
let initialized: Promise<Backend> | undefined;

/** Initializes the WASM bindings once, from `module`, else the binary the import condition picks. */
export function initWasm(module?: BackendModule, fallback?: BackendModule): Promise<Backend> {
  initialized ??= instantiate(module ?? fallback).catch((error) => {
    initialized = undefined;
    throw error;
  });

  return initialized;
}

async function instantiate(source: BackendModule | undefined): Promise<Backend> {
  const resolved = typeof source === "function" ? await source() : await source;
  const input =
    resolved !== null && typeof resolved === "object" && "default" in resolved
      ? resolved.default
      : resolved;

  await initBindings(input ? { module_or_path: input } : undefined);

  return { Renderer, setGlyphCacheMaxBytes };
}
