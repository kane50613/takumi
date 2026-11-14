import { fetchFollowed, type FetchLike, type FetchOptions, readBodyLimited } from "../fetch";
import type { CacheStoreStats } from "../types";
import { FetchDeadline } from "./deadline";

/** Bytes a store keeps for the URL they were requested at. */
export type StoredDownload = {
  bytes: ArrayBuffer;
  /** The URL the bytes came from after redirects, which a later request's `allowUrl` checks. */
  url: string;
};

/** What a store reports about the bytes it holds, in the units `cacheStats().store` uses. */
export type DownloadStoreUsage = {
  /** The bytes held now. */
  current: number;
  /** The most bytes the store holds. */
  max: number;
  /** Downloads dropped to stay within `max`. */
  evictions: number;
};

/**
 * Where downloaded bytes are kept, by the URL they were requested at. The store decides what to
 * keep and when to drop it. Either method may return a promise.
 */
export interface DownloadStore {
  /** The download kept for `url`, or `undefined` when there is none. */
  get(url: string): StoredDownload | undefined | PromiseLike<StoredDownload | undefined>;
  /** Offers `download`, requested at `url`, for the store to keep. */
  set(url: string, download: StoredDownload): void | PromiseLike<void>;
  /** What the store holds now. Without it, `cacheStats().store` reports its size as `null`. */
  usage?(): DownloadStoreUsage;
}

/** The bytes a {@link MemoryStore} holds when its options leave `maxBytes` out. */
const defaultMaxStoreBytes = 32 * 1024 * 1024;

/**
 * A {@link DownloadStore} in memory that drops the downloads used least recently once it holds
 * more than `maxBytes`. Sizes are estimates that add two bytes for each character of the URLs to the bytes.
 */
export class MemoryStore implements DownloadStore {
  /** The most bytes the store holds. `0` keeps nothing. */
  readonly maxBytes: number;
  private readonly entries = new Map<string, { download: StoredDownload; bytes: number }>();
  private current = 0;
  private evictions = 0;

  /** A store holding at most `maxBytes`, 32 MiB by default. */
  constructor(options: { maxBytes?: number } = {}) {
    this.maxBytes = Math.max(0, options.maxBytes ?? defaultMaxStoreBytes);
  }

  get(url: string): StoredDownload | undefined {
    const entry = this.entries.get(url);

    if (!entry) {
      return undefined;
    }

    this.entries.delete(url);
    this.entries.set(url, entry);

    return entry.download;
  }

  set(url: string, download: StoredDownload): void {
    this.delete(url);

    const bytes = download.bytes.byteLength + (url.length + download.url.length) * 2;

    if (bytes > this.maxBytes) {
      return;
    }

    this.entries.set(url, { download, bytes });
    this.current += bytes;

    for (const oldest of this.entries.keys()) {
      if (this.current <= this.maxBytes) {
        break;
      }

      this.delete(oldest);
      this.evictions += 1;
    }
  }

  usage(): DownloadStoreUsage {
    return { current: this.current, max: this.maxBytes, evictions: this.evictions };
  }

  private delete(url: string) {
    const entry = this.entries.get(url);

    if (entry) {
      this.current -= entry.bytes;
      this.entries.delete(url);
    }
  }
}

/** One value for each object `create` is called with, made on the first call. */
export function perObject<K extends object, T>(create: (key: K) => T): (key: K) => T {
  const values = new WeakMap<K, T>();

  return (key) => {
    let value = values.get(key);

    if (value === undefined) {
      value = create(key);
      values.set(key, value);
    }

    return value;
  };
}

/** The store `googleFonts`, `fontFromUrl`, and emoji images use when their options name none. */
// One for each `fetch` function, since a custom `fetch` can answer the same URL differently.
export const defaultStore: (fetch: FetchLike) => MemoryStore = perObject(() => new MemoryStore());

/**
 * One download that requests with the same `fetch`, `allowUrl`, and `maxBytes` share. Each request
 * stops waiting on its own `signal` and `timeout`, and the download stops once no request waits on it.
 */
class Flight {
  readonly done: Promise<ArrayBuffer>;
  private readonly controller = new AbortController();
  private waiting = 0;
  private settled = false;

  constructor(
    private readonly policy: Pick<FetchOptions, "fetch" | "allowUrl">,
    private readonly maxBytes: number,
    run: (signal: AbortSignal) => Promise<ArrayBuffer>,
    private readonly abandon: () => void,
  ) {
    this.done = run(this.controller.signal);
    const settle = () => {
      this.settled = true;
    };

    this.done.then(settle, settle);
  }

  /** Whether a request with `options` and `maxBytes` may wait on this download instead of starting its own. */
  serves(options: FetchOptions, maxBytes: number): boolean {
    return (
      options.fetch === this.policy.fetch &&
      options.allowUrl === this.policy.allowUrl &&
      maxBytes === this.maxBytes
    );
  }

  wait(options: FetchOptions): Promise<ArrayBuffer> {
    this.waiting += 1;

    return new FetchDeadline(options).waitFor(this.done).finally(() => {
      this.waiting -= 1;

      if (this.waiting === 0 && !this.settled) {
        this.abandon();
        this.controller.abort();
      }
    });
  }
}

/** The downloads in flight for one store, and how lookups in it went. */
type StoreState = { flights: Map<string, Flight[]>; hits: number; misses: number };

const stateOf = perObject((_store: DownloadStore): StoreState => ({
  flights: new Map(),
  hits: 0,
  misses: 0,
}));

/** Whether `value` is a promise, from this realm or another, or another thenable. */
function isPromiseLike<T>(value: T | PromiseLike<T>): value is PromiseLike<T> {
  return (
    typeof value === "object" &&
    value !== null &&
    "then" in value &&
    typeof value.then === "function"
  );
}

/** What `store` holds for `url`, a store that throws counting as holding nothing. */
function lookup(
  store: DownloadStore,
  url: string,
): StoredDownload | undefined | Promise<StoredDownload | undefined> {
  try {
    const found = store.get(url);

    return isPromiseLike(found) ? Promise.resolve(found).catch(() => undefined) : found;
  } catch {
    return undefined;
  }
}

/** Offers `download` to `store`, ignoring a store that fails to keep it. */
function offer(store: DownloadStore, url: string, download: StoredDownload) {
  try {
    const kept = store.set(url, download);

    if (isPromiseLike(kept)) {
      Promise.resolve(kept).catch(() => {});
    }
  } catch {
    // The download still serves the requests that made it.
  }
}

function withinMaxBytes(bytes: ArrayBuffer, maxBytes: number): ArrayBuffer {
  if (bytes.byteLength > maxBytes) {
    throw new Error(`Response exceeds ${maxBytes} bytes`);
  }

  return bytes;
}

/**
 * Fetches `url` through `store`. A kept download serves the request when its `allowUrl` accepts
 * the URL the bytes came from, and a request it refuses fetches again without dropping it.
 */
export async function fetchStored(
  url: string,
  options: FetchOptions & { init?: RequestInit },
  store: DownloadStore,
  maxBytes: number,
): Promise<ArrayBuffer> {
  if (options.allowUrl && !options.allowUrl(url)) {
    throw new Error(`URL blocked by allowUrl policy: ${url}`);
  }

  options.signal?.throwIfAborted();

  const state = stateOf(store);
  const flights = state.flights.get(url) ?? [];
  const shared = flights.find((flight) => flight.serves(options, maxBytes));

  if (shared) {
    return shared.wait(options);
  }

  const leave = () => {
    const remaining = (state.flights.get(url) ?? []).filter((each) => each !== flight);

    if (remaining.length > 0) {
      state.flights.set(url, remaining);
    } else {
      state.flights.delete(url);
    }
  };
  const use = (stored: StoredDownload | undefined, signal: AbortSignal) => {
    if (stored && (!options.allowUrl || options.allowUrl(stored.url))) {
      state.hits += 1;

      return Promise.resolve(stored.bytes).then((bytes) => withinMaxBytes(bytes, maxBytes));
    }

    state.misses += 1;

    return fetchFollowed(url, { ...options, signal, timeout: 0 }).then(async (followed) => {
      const bytes = await readBodyLimited(followed.response, maxBytes);

      // A download nobody waits on any more may finish after a newer one replaced it in the store.
      if (!signal.aborted) {
        offer(store, url, { bytes, url: followed.url });
      }

      return bytes;
    });
  };
  const flight = new Flight(
    { fetch: options.fetch, allowUrl: options.allowUrl },
    maxBytes,
    (signal) => {
      const found = lookup(store, url);

      return found instanceof Promise
        ? found.then((stored) => use(stored, signal))
        : use(found, signal);
    },
    leave,
  );

  state.flights.set(url, [...flights, flight]);
  flight.done.then(leave, leave);

  return flight.wait(options);
}

/**
 * What `store` holds and how the lookups takumi made in it went.
 *
 * @internal
 */
export function storeStats(store: DownloadStore): CacheStoreStats {
  const usage = store.usage?.();
  const { hits, misses } = stateOf(store);

  return {
    current: usage?.current ?? null,
    max: usage?.max ?? null,
    hits,
    misses,
    evictions: usage?.evictions ?? null,
  };
}
