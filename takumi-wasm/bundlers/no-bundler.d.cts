import type init from "../dist/export.cjs";
import type { initSync as initSyncImpl, Renderer as RendererImpl } from "../dist/export.cjs";

export * from "../dist/export.cjs";

/** @deprecated Import `Renderer` from `@takumi-rs/wasm` instead. Will be removed in v3. */
export declare const Renderer: typeof RendererImpl;
/** @deprecated Import `Renderer` from `@takumi-rs/wasm` instead. Will be removed in v3. */
export type Renderer = RendererImpl;
/** @deprecated Import `initSync` from `@takumi-rs/wasm` instead. Will be removed in v3. */
export declare const initSync: typeof initSyncImpl;
/** @deprecated Import the default export from `@takumi-rs/wasm` instead. Will be removed in v3. */
declare const deprecatedInit: typeof init;
export default deprecatedInit;
