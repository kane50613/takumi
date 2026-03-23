import * as wasm from "@takumi-rs/wasm";

export type Imports = Awaited<ReturnType<typeof getImportsImpl>>;

let importPromise: Promise<Imports> | null = null;

export function getImports(module?: wasm.InitInput) {
  importPromise ??= getImportsImpl(module);

  return importPromise;
}

async function getImportsImpl(module?: wasm.InitInput) {
  if (module) {
    return initializeWasm(module);
  }

  // Have to make it a separated variable outside of the if block so esbuild won't try to statically analyze the import.
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
  } catch (error) {
    throw new Error(
      "Couldn't automatically resolve Takumi native bindings. Please specify the module option with the WASM module.",
      {
        cause: error,
      },
    );
  }
}

async function importWasm() {
  // Vite path
  if (import.meta.env?.BASE_URL && !import.meta.env?.SSR) {
    return import("@takumi-rs/wasm/vite");
  }

  // Cloudflare Workers/esbuild path
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
}
