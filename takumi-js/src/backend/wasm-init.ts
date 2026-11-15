import { init as initBindings, Renderer, setGlyphCacheMaxBytes } from "@takumi-rs/wasm";
import { SharedLoad } from "./shared-load";
import type { Backend, BackendModule } from "./types";

// Overlapping `init` calls each instantiate the binary, and the later one strands every renderer
// made on the earlier one, so all callers share the first.
const initialized = new SharedLoad<Backend>();

/** Initializes the WASM bindings once, from `module`, else the binary the import condition picks. */
export function initWasm(module?: BackendModule, fallback?: BackendModule): Promise<Backend> {
  return initialized.get(() => instantiate(module ?? fallback));
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
