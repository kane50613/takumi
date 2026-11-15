import { loadBackend } from "../backend/wasm";

export { default } from "@takumi-rs/wasm/auto";
export * from "@takumi-rs/wasm";

/** Resolves once the bindings are initialized from the binary the import conditions pick. */
export async function ready(): Promise<void> {
  await loadBackend();
}
