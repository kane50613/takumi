import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import type { RenderOptionsWithoutRenderer } from "./render";

export type ManagedRendererOptions = {
  fonts?: napi.FontLoader[];
  /**
   * Only supported by the native `@takumi-rs/core` renderer.
   * This option is ignored when using the WASM renderer.
   */
  loadDefaultFonts?: boolean;
  persistentImages?: napi.ImageSourceLoader[];
  /**
   * @description The WebAssembly module to use for the renderer. If not provided, the default resolving strategy will be used.
   */
  module?: wasm.InitInput | Promise<wasm.InitInput> | { default: wasm.InitInput };
};

export async function loadRendererResources(
  renderer: napi.Renderer | wasm.Renderer,
  options: RenderOptionsWithoutRenderer | undefined,
) {
  const tasks: Promise<unknown>[] = [];

  if (options?.fonts && options.fonts.length > 0) {
    if (options.fonts.length > 0) {
      tasks.push(renderer.loadFonts(options.fonts));
    }
  }

  if (options?.persistentImages && options.persistentImages.length > 0) {
    tasks.push(
      ...options.persistentImages.map((image) =>
        Promise.resolve(renderer.putPersistentImage(image, options.signal)),
      ),
    );
  }

  if (tasks.length > 0) {
    await Promise.all(tasks);
  }
}
