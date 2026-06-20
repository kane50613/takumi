import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";

export type ManagedRendererOptions = {
  fonts?: napi.FontLoader[];
  /**
   * @description The WebAssembly module to use for the renderer. If not provided, the default resolving strategy will be used.
   */
  module?: wasm.InitInput | Promise<wasm.InitInput> | { default: wasm.InitInput };
};
