import url from "../pkg/takumi_wasm_bg.wasm?url";

// `?url` is a browser path; map it to a file for server reads. Vite emits the asset next to the
// importing server chunk, so resolve relative to import.meta.url, not the framework output dir.
async function processUrl() {
  if (typeof process !== "undefined" && process.versions?.node != null) {
    const { readFile } = await import("node:fs/promises");
    const path = decodeURIComponent(url.replace(/[?#].*$/, ""));

    // Dev SSR serves an `/@fs/<abs-path>` URL.
    if (path.startsWith("/@fs/")) {
      return readFile(path.slice("/@fs".length));
    }

    if (!path.startsWith("/")) {
      return readFile(new URL(path, import.meta.url));
    }

    const basename = path.slice(path.lastIndexOf("/") + 1);
    const candidates = [`./${basename}`, `../client${path}`, `../../client${path}`];

    let lastError;
    for (const candidate of candidates) {
      try {
        return await readFile(new URL(candidate, import.meta.url));
      } catch (error) {
        if (error && typeof error === "object" && "code" in error && error.code === "ENOENT") {
          lastError = error;
          continue;
        }

        throw error;
      }
    }

    throw new Error(`Unable to locate Takumi WASM asset for SSR: ${url}`, { cause: lastError });
  }

  return fetch(new URL(url, import.meta.url)).then((response) => response.arrayBuffer());
}

export default processUrl();
