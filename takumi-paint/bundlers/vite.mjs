import url from "../pkg/takumi_paint_wasm_bg.wasm?url";
import * as wasm from "../dist/export.mjs";

// `?url` is a browser path; map it to a file for server reads. Vite emits the asset next to the
// importing server chunk, so resolve relative to import.meta.url, not the framework output dir.
async function resolveWasm() {
  if (typeof process !== "undefined" && process.versions?.node != null) {
    // Nitro 2 traces this file but its output loads node.mjs at runtime, so name it for the tracer.
    // Vite's module runner throws on import.meta.resolve, and the hint is only for the tracer.
    try {
      import.meta.resolve("./node.mjs");
    } catch {}

    const { readFile } = process.getBuiltinModule("node:fs/promises");
    const path = decodeURIComponent(url.replace(/[?#].*$/, ""));

    // Dev SSR serves an `/@fs/<abs-path>` URL.
    if (path.startsWith("/@fs/")) {
      return readFile(path.slice("/@fs".length));
    }

    if (!path.startsWith("/")) {
      return readFile(new URL(path, import.meta.url));
    }

    // The last candidate is dev SSR serving a package inside the Vite root by its root-relative path.
    const basename = path.slice(path.lastIndexOf("/") + 1);
    const candidates = [
      `./${basename}`,
      `.${path}`,
      `../client${path}`,
      `../../client${path}`,
      `../pkg/${basename}`,
    ];

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

wasm.initSync({ module: await resolveWasm() });

export * from "../dist/export.mjs";
export default wasm.default;
