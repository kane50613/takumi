import * as wasm from "@takumi-rs/wasm";
import type { Backend, BackendModule } from "./types";

/**
 * Initializes the WASM bindings, preferring a caller-supplied `module` and
 * falling back to the per-bundler binary picked by the import condition.
 * `@takumi-rs/wasm` guards against double init, so a binary already loaded by
 * `@takumi-rs/wasm/auto` (e.g. on Deno) makes this a no-op.
 */
export async function initWasm(
  fallback: BackendModule,
  module: BackendModule | undefined,
): Promise<Backend> {
  const source = module ?? fallback;
  const resolved = typeof source === "function" ? await source() : await source;
  const input =
    resolved !== undefined && typeof resolved === "object" && "default" in resolved
      ? resolved.default
      : resolved;

  await wasm.default(input ? { module_or_path: input } : undefined);

  return wasm;
}
