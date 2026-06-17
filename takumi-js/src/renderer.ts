import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import type { RenderOptionsWithoutRenderer } from "./render";

export type ManagedRendererOptions = {
  fonts?: napi.FontLoader[];
  /**
   * Whether to load the embedded default fonts.
   * Defaults to `false` when `fonts` are provided.
   */
  loadDefaultFonts?: boolean;
  /**
   * @description The WebAssembly module to use for the renderer. If not provided, the default resolving strategy will be used.
   */
  module?: wasm.InitInput | Promise<wasm.InitInput> | { default: wasm.InitInput };
};

/**
 * Mirrors the renderer constructor behavior: when custom fonts are provided,
 * default fonts are disabled so they can't shadow user fonts through generic
 * family resolution (e.g. `sans-serif` resolving to the embedded font).
 */
export function shouldLoadDefaultFonts(
  options: RenderOptionsWithoutRenderer | undefined,
): boolean | undefined {
  return options?.loadDefaultFonts ?? (options?.fonts?.length ? false : undefined);
}

export async function loadRendererResources(
  renderer: napi.Renderer | wasm.Renderer,
  options: RenderOptionsWithoutRenderer | undefined,
) {
  if (options?.fonts && options.fonts.length > 0) {
    await renderer.registerFonts(options.fonts);
  }
}
