import initBindings, { Renderer, setGlyphCacheMaxBytes } from "@takumi-rs/wasm";
import type { Backend, BackendModule } from "./types";

/** Initializes the WASM bindings from `module`, else the binary the import condition picks. */
export async function initWasm(module?: BackendModule, fallback?: BackendModule): Promise<Backend> {
  const source = module ?? fallback;
  const resolved = typeof source === "function" ? await source() : await source;
  const input =
    resolved !== null && typeof resolved === "object" && "default" in resolved
      ? resolved.default
      : resolved;

  await initBindings(input ? { module_or_path: input } : undefined);

  return { Renderer, setGlyphCacheMaxBytes };
}
