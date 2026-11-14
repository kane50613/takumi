import {
  type CacheOptions,
  type CacheStoreStats,
  type DownloadStore,
  MemoryStore,
  type RendererOptions,
  storeStats,
} from "@takumi-rs/helpers";

/**
 * The store a renderer keeps the bytes it downloads in, as `cache.store` sets it: a `MemoryStore`
 * of its own when left out, none when `null`.
 *
 * @internal
 */
export class RendererStore {
  #current: DownloadStore | null;
  #default: MemoryStore | undefined;

  constructor(options?: CacheOptions) {
    this.#current = this.#resolve(options?.store);
  }

  /** The store downloads go through now, or `null` when they are not kept. */
  get current(): DownloadStore | null {
    return this.#current;
  }

  /**
   * Applies `options` as if the renderer were created with them, handing the rest to `setMemory`
   * first so options it rejects change nothing. Going back to the default store finds what it held.
   */
  apply(options: CacheOptions, setMemory: (memory: Omit<CacheOptions, "store">) => void): void {
    const { store, ...memory } = options;

    setMemory(memory);
    this.#current = this.#resolve(store);
  }

  /** What the store holds and how lookups in it went, or `null` without one. */
  stats(): CacheStoreStats | null {
    return this.#current && storeStats(this.#current);
  }

  #resolve(store: DownloadStore | null | undefined): DownloadStore | null {
    return store === undefined ? (this.#default ??= new MemoryStore()) : store;
  }
}

/**
 * `options` without `cache.store`, which stays in JavaScript.
 *
 * @internal
 */
export function withoutStore(options?: RendererOptions): RendererOptions | undefined {
  if (!options?.cache) {
    return options;
  }

  const { store: _store, ...cache } = options.cache;

  return { ...options, cache };
}
