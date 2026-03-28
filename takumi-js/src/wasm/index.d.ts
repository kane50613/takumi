export * from "@takumi-rs/wasm";

import type { InitInput } from "@takumi-rs/wasm";

declare const wasm: InitInput;
export default wasm;

/**
 * Initializes the WASM module from automatically resolved source.
 * @param input wasm source
 */
export function init(input?: InitInput): Promise<void>;
