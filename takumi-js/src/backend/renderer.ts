import { loadBackend } from "#backend";
import { managedCacheOptions } from "../cache";
import { initWasm } from "./wasm-init";
import type { Backend, BackendModule, LoadBackend } from "./types";

export type Renderer = InstanceType<Backend["Renderer"]>;

export class RendererProvider {
  private readonly load: LoadBackend;
  private backend: Promise<Backend> | undefined;
  private renderer: Renderer | undefined;

  constructor(load: LoadBackend) {
    this.load = load;
  }

  async get(module?: BackendModule): Promise<Renderer> {
    this.backend ??= this.load(module).catch((error) => {
      this.backend = undefined;
      throw error;
    });
    const backend = await this.backend;
    return (this.renderer ??= new backend.Renderer({ cache: managedCacheOptions() }));
  }
}

export const defaultRenderer: RendererProvider = new RendererProvider((module) =>
  module === undefined ? loadBackend() : initWasm(module),
);
