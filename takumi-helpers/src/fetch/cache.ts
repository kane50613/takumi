import type { FetchLike } from "../fetch";

type CacheLimits<T> = {
  maxBytes: number;
  maxEntries: number;
  /** Bytes a settled value holds. */
  sizeOf: (value: T) => number;
};

/**
 * A least-recently-used cache of pending fetches by URL, bounded by entry count and by the bytes
 * each one settles to. A fetch that fails leaves it.
 */
export class BoundedCache<T> {
  private readonly entries = new Map<string, { pending: Promise<T>; bytes: number }>();
  private bytes = 0;

  constructor(private readonly limits: CacheLimits<T>) {}

  get(url: string): Promise<T> | undefined {
    const entry = this.entries.get(url);

    if (entry) {
      this.entries.delete(url);
      this.entries.set(url, entry);
    }

    return entry?.pending;
  }

  set(url: string, pending: Promise<T>): void {
    this.delete(url);

    const entry = { pending, bytes: url.length * 2 };

    this.entries.set(url, entry);
    this.bytes += entry.bytes;
    this.trim();
    pending.then(
      (value) => {
        if (this.entries.get(url) !== entry) {
          return;
        }

        const bytes = this.limits.sizeOf(value);

        entry.bytes += bytes;
        this.bytes += bytes;

        if (entry.bytes > this.limits.maxBytes) {
          this.delete(url);
        }

        this.trim();
      },
      () => {
        if (this.entries.get(url) === entry) {
          this.delete(url);
        }
      },
    );
  }

  delete(url: string): void {
    const entry = this.entries.get(url);

    if (entry) {
      this.bytes -= entry.bytes;
      this.entries.delete(url);
    }
  }

  private trim() {
    for (const url of this.entries.keys()) {
      if (this.entries.size <= this.limits.maxEntries && this.bytes <= this.limits.maxBytes) {
        break;
      }

      this.delete(url);
    }
  }
}

/** One value for each `fetch` function, since a custom `fetch` can answer the same URL differently. */
export function perFetch<T>(create: () => T): (fetch: FetchLike) => T {
  const byFetch = new WeakMap<FetchLike, T>();

  return (fetch) => {
    let value = byFetch.get(fetch);

    if (!value) {
      value = create();
      byFetch.set(fetch, value);
    }

    return value;
  };
}
