import { loadBackend } from "#backend";
import { applyGlyphCacheMaxBytes } from "../glyph-cache";
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
    return (this.renderer ??= new backend.Renderer());
  }
}

export const defaultRenderer = new RendererProvider(async (module) => {
  const backend = await (module === undefined
    ? loadBackend()
    : import("./wasm-init").then(({ initWasm }) => initWasm(module)));
  applyGlyphCacheMaxBytes(backend);
  return backend;
});
