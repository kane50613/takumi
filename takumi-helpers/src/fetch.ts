const defaultFetchTimeout = 30_000;
const maxRedirectHops = 5;
const maxFetchAttempts = 3;
const maxRetryDelay = 1000;
const retryableStatuses = new Set([408, 429, 500, 502, 503, 504]);
export const defaultMaxFetchBytes = 32 * 1024 * 1024;

export type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export type FetchOptions = {
  /** Custom fetch implementation. @default globalThis.fetch */
  fetch?: FetchLike;
  /** Total request timeout, including retries; `0` or negative disables it. @default 30000 */
  timeout?: number;
  /** Caller abort signal, combined with the timeout. */
  signal?: AbortSignal;
  /** Reject bodies larger than this many bytes. @default 33554432 (32 MiB) */
  maxBytes?: number;
  /** Return false to skip fetching a URL (e.g. SSRF allowlist). */
  allowUrl?: (url: string) => boolean;
};

/** Fetches a URL, applying a timeout signal and throwing on a non-OK status. */
export async function fetchOk(url: string, options: FetchOptions & { init?: RequestInit } = {}) {
  return new FetchRequest(url, options).send();
}

class FetchRequest {
  private readonly url: string;
  private readonly allowUrl: FetchOptions["allowUrl"];
  private readonly fetchImpl: FetchLike;
  private readonly init: RequestInit;

  constructor(url: string, options: FetchOptions & { init?: RequestInit }) {
    if (options.allowUrl && !options.allowUrl(url)) {
      throw new Error(`URL blocked by allowUrl policy: ${url}`);
    }
    this.url = url;
    this.allowUrl = options.allowUrl;
    this.fetchImpl = options.fetch ?? globalThis.fetch;
    const timeout = options.timeout ?? defaultFetchTimeout;
    const timeoutSignal = timeout <= 0 ? undefined : AbortSignal.timeout(timeout);
    const signals = [options.signal, options.init?.signal, timeoutSignal].filter(
      (signal): signal is AbortSignal => signal != null,
    );
    this.init = {
      ...options.init,
      signal: signals.length ? AbortSignal.any(signals) : undefined,
    };
  }

  async send() {
    const response = this.allowUrl
      ? await this.followRedirects(this.allowUrl)
      : await this.attempt(this.url, this.init);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} ${response.statusText} fetching ${this.url}`);
    }
    return response;
  }

  private async attempt(url: string, init: RequestInit): Promise<Response> {
    const method = init.method?.toUpperCase() ?? "GET";
    const canRetry = method === "GET" || method === "HEAD";
    for (let attempt = 0; ; attempt++) {
      init.signal?.throwIfAborted();
      let delay = 100 * 2 ** attempt;
      try {
        const response = await this.fetchImpl.call(undefined, url, init);
        init.signal?.throwIfAborted();
        if (
          !canRetry ||
          attempt === maxFetchAttempts - 1 ||
          !retryableStatuses.has(response.status)
        ) {
          return response;
        }
        const retryAfter = response.headers.get("retry-after");
        if (retryAfter !== null) {
          const seconds = Number(retryAfter);
          const milliseconds = Number.isFinite(seconds)
            ? seconds * 1000
            : Date.parse(retryAfter) - Date.now();
          if (Number.isFinite(milliseconds)) {
            delay = Math.max(delay, milliseconds);
          }
        }
        if (delay > maxRetryDelay) {
          return response;
        }
        void response.body?.cancel().catch(() => {});
      } catch (error) {
        init.signal?.throwIfAborted();
        if (
          !canRetry ||
          attempt === maxFetchAttempts - 1 ||
          !(error instanceof Error) ||
          (error.name !== "TypeError" && error.name !== "TimeoutError")
        ) {
          throw error;
        }
      }
      await this.wait(delay);
    }
  }

  private wait(delay: number): Promise<void> {
    const signal = this.init.signal;
    signal?.throwIfAborted();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        signal?.removeEventListener("abort", abort);
        resolve();
      }, delay);
      function abort() {
        clearTimeout(timer);
        reject(signal?.reason);
      }
      signal?.addEventListener("abort", abort, { once: true });
    });
  }

  private async followRedirects(allowUrl: (url: string) => boolean): Promise<Response> {
    let current = this.url;
    for (let hop = 0; hop < maxRedirectHops; hop++) {
      const response = await this.attempt(current, { ...this.init, redirect: "manual" });
      const location = response.headers.get("location");
      if (response.status < 300 || response.status >= 400 || !location) {
        return response;
      }

      await response.body?.cancel().catch(() => {});
      current = new URL(location, current).toString();
      if (!allowUrl(current)) {
        throw new Error(`URL blocked by allowUrl policy: ${current}`);
      }
    }
    throw new Error(`Too many redirects fetching ${this.url}`);
  }
}

/** Reads a response body, rejecting once it exceeds `maxBytes` (by content-length or streamed size). */
export async function readBodyLimited(response: Response, maxBytes: number): Promise<ArrayBuffer> {
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > maxBytes) {
    throw new Error(`Response exceeds ${maxBytes} bytes (content-length ${declared})`);
  }

  const body = response.body;
  if (!body) {
    return response.arrayBuffer();
  }

  const reader = body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }

    total += value.byteLength;
    if (total > maxBytes) {
      await reader.cancel().catch(() => {});
      throw new Error(`Response exceeds ${maxBytes} bytes`);
    }
    chunks.push(value);
  }

  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return out.buffer;
}
