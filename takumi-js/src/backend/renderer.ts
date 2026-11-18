import type { CacheStats } from "@takumi-rs/core";
import { loadBackend } from "#backend";
import { applyGlyphCacheMaxBytes } from "../glyph-cache";
import { SharedLoad } from "./shared-load";
import { initWasm } from "./wasm-init";
import type { Backend, BackendModule, LoadBackend } from "./types";

export type Renderer = InstanceType<Backend["Renderer"]>;

export class RendererProvider {
  private readonly load: LoadBackend;
  private readonly backend = new SharedLoad<Backend>();
  private renderer: Renderer | undefined;

  constructor(load: LoadBackend) {
    this.load = load;
  }

  /** What the renderer's cache holds, none before the renderer exists. */
  cacheStats(): CacheStats | undefined {
    return this.renderer?.cacheStats();
  }

  async get(module?: BackendModule): Promise<Renderer> {
    const backend = await this.backend.get(() => this.load(module));
    return (this.renderer ??= new backend.Renderer());
  }
}

export const defaultRenderer: RendererProvider = new RendererProvider(async (module) => {
  const backend = await (module === undefined ? loadBackend() : initWasm(module));
  applyGlyphCacheMaxBytes(backend);
  return backend;
});
