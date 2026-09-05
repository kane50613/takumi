import type { FetchOptions } from "../fetch";

const defaultFetchTimeout = 30_000;

export class FetchDeadline {
  readonly signal: AbortSignal | undefined;

  constructor(options: FetchOptions, initSignal?: AbortSignal | null) {
    const timeout = options.timeout ?? defaultFetchTimeout;
    const signals = [
      options.signal,
      initSignal,
      timeout <= 0 ? undefined : AbortSignal.timeout(timeout),
    ].filter((signal): signal is AbortSignal => signal != null);

    this.signal = signals.length ? AbortSignal.any(signals) : undefined;
  }

  waitFor<T>(promise: Promise<T>): Promise<T> {
    const signal = this.signal;

    if (!signal) {
      return promise;
    }

    return new Promise((resolve, reject) => {
      function abort() {
        signal?.removeEventListener("abort", abort);
        reject(signal?.reason);
      }
      signal.addEventListener("abort", abort, { once: true });
      promise.then(
        (value) => {
          signal.removeEventListener("abort", abort);
          resolve(value);
        },
        (error) => {
          signal.removeEventListener("abort", abort);
          reject(error);
        },
      );
      if (signal.aborted) {
        abort();
      }
    });
  }
}
