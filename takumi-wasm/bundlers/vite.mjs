import url from "../pkg/takumi_wasm_bg.wasm?url";

async function processUrl() {
  if (typeof process !== "undefined" && process.versions?.node != null) {
    const { readFile } = await import("node:fs/promises");

    if (!url.startsWith("/")) {
      return readFile(new URL(url, import.meta.url));
    }

    for (const candidate of [`../client${url}`, `../../client${url}`]) {
      try {
        return await readFile(new URL(candidate, import.meta.url));
      } catch (error) {
        if (error && typeof error === "object" && "code" in error && error.code === "ENOENT") {
          continue;
        }

        throw error;
      }
    }

    throw new Error(`Unable to locate Takumi WASM asset for SSR: ${url}`);
  }

  return fetch(new URL(url, import.meta.url)).then((response) => response.arrayBuffer());
}

export default processUrl();
