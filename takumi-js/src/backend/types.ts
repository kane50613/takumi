import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";

type RendererBindings = "Renderer" | "setGlyphCacheMaxBytes";

/** The bindings a renderer needs, from whichever backend the import conditions selected. */
export type Backend = Pick<typeof napi, RendererBindings> | Pick<typeof wasm, RendererBindings>;

/** A WASM binary, or something that resolves to one, for manual initialization. */
export type BackendModule =
  | wasm.InitInput
  | { default: wasm.InitInput }
  | Promise<wasm.InitInput | { default: wasm.InitInput }>
  | (() => Promise<wasm.InitInput | { default: wasm.InitInput }>);

export type LoadBackend = (module?: BackendModule) => Promise<Backend>;
