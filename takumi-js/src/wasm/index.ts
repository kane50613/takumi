import { Renderer } from "@takumi-rs/wasm";
import autoModule from "@takumi-rs/wasm/auto";
import { initWasm } from "../backend/wasm-init";
import type { BackendModule } from "../backend/types";

export { default } from "@takumi-rs/wasm/auto";
export { default as init } from "@takumi-rs/wasm";
export * from "@takumi-rs/wasm";
export type { BackendModule };

/**
 * Initializes the WASM bindings and returns a renderer to pass as `renderer`.
 * Forces WASM on a runtime the import conditions map to the native backend but
 * that can't load the addon (WebContainer, an unsupported platform).
 */
export async function createRenderer(module?: BackendModule) {
  await initWasm(module, autoModule);

  return new Renderer();
}
