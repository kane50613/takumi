import type { CacheOptions, CacheStats } from "@takumi-rs/core";
import { loadBackend } from "#backend";
import { initWasm } from "./wasm-init";
import type { Backend, BackendModule, LoadBackend } from "./types";

export type Renderer = InstanceType<Backend["Renderer"]>;

export class RendererProvider {
  private readonly load: LoadBackend;
  private backend: Promise<Backend> | undefined;
  private renderer: Renderer | undefined;
  private options: CacheOptions | undefined;

  constructor(load: LoadBackend) {
    this.load = load;
  }

  /** The cache options the renderer was last given, or is created with. */
  get cacheOptions(): CacheOptions | undefined {
    return this.options;
  }

  /** Applies `options` to the renderer when it exists, and keeps them for one created later. */
  setCacheOptions(options: CacheOptions): void {
    this.options = options;
    this.renderer?.setCacheOptions(options);
  }

  /** What the renderer's cache holds, none before the renderer exists. */
  cacheStats(): CacheStats | undefined {
    return this.renderer?.cacheStats();
  }

  async get(module?: BackendModule): Promise<Renderer> {
    this.backend ??= this.load(module).catch((error) => {
      this.backend = undefined;
      throw error;
    });
    const backend = await this.backend;
    return (this.renderer ??= new backend.Renderer({ cache: this.options }));
  }
}

export const defaultRenderer: RendererProvider = new RendererProvider((module) =>
  module === undefined ? loadBackend() : initWasm(module),
);
