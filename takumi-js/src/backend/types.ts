import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";

/** The bindings namespace, whichever backend the import conditions selected. */
export type Backend = typeof napi | typeof wasm;

/** A WASM binary, or something that resolves to one, for manual initialization. */
export type BackendModule =
  | wasm.InitInput
  | { default: wasm.InitInput }
  | Promise<wasm.InitInput | { default: wasm.InitInput }>
  | (() => Promise<wasm.InitInput | { default: wasm.InitInput }>);

export type LoadBackend = (module?: BackendModule) => Promise<Backend>;
