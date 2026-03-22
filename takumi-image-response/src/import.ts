import * as wasm from "@takumi-rs/wasm";

export type Imports = Awaited<ReturnType<typeof getImports>>;

export async function getImports(module?: wasm.InitInput) {
  if (module) {
    return initializeWasm(module);
  }

  const core = "@takumi-rs/core";
  if (typeof process !== "undefined" && process.env.NEXT_RUNTIME !== "edge") {
    return import(/* @__PURE__ */ /* @vite-ignore */ core) as Promise<
      typeof import("@takumi-rs/core")
    >;
  }

  const importedModule = await importWasm();
  return initializeWasm(
    importedModule && "default" in importedModule ? importedModule.default : importedModule,
  );
}

async function initializeWasm(module?: wasm.InitInput) {
  try {
    await wasm.default(module ? { module_or_path: module } : undefined);

    return wasm;
  } catch (e) {
    console.error("Failed to initialize WASM module:", e);
    throw new Error(
      "Failed to resolve Takumi native bindigns automatically. Please provide the `module` option with the WASM module.",
    );
  }
}

async function importWasm() {
  // Cloudflare Workers path
  if (typeof navigator !== "undefined" && navigator.userAgent === "Cloudflare-Workers") {
    return import(
      /* @__PURE__ */ /* @vite-ignore */ /* webpackIgnore: true */ /* turbopackIgnore: true */ "@takumi-rs/wasm/takumi_wasm_bg.wasm"
    ) as Promise<typeof import("@takumi-rs/wasm/takumi_wasm_bg.wasm")>;
  }

  // Next.js path
  const nextPath = "@takumi-rs/wasm/next";
  if (typeof process !== "undefined" && process.env.NEXT_RUNTIME) {
    return import(/* @__PURE__ */ /* @vite-ignore */ nextPath) as Promise<
      typeof import("@takumi-rs/wasm/next")
    >;
  }

  // Vite path
  const vitePath = "@takumi-rs/wasm/takumi_wasm_bg.wasm?url";
  if (import.meta.env.MODE) {
    const url: string = await import(
      /* @__PURE__ */ /* webpackIgnore: true */ /* turbopackIgnore: true */ vitePath
    );

    return fetch(url).then((res) => res.arrayBuffer());
  }
}
