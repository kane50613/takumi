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

  const importedModule = await importBindings();
  if (importedModule && "Renderer" in importedModule) {
    return importedModule;
  }

  return initializeWasm(importedModule);
}

async function initializeWasm(module?: wasm.InitInput | { default: wasm.InitInput }) {
  const wasmModule =
    module !== undefined && typeof module === "object" && "default" in module
      ? module.default
      : module;

  try {
    await wasm.default(wasmModule ? { module_or_path: wasmModule } : undefined);

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

async function importBindings() {
  if (shouldSkipCoreImport()) {
    return importWasmBindings();
  }

  try {
    return await import("@takumi-rs/core");
  } catch (error) {
    console.warn(
      "Unable to import @takumi-rs/core. Falling back to auto-detection of WASM bindings.",
      {
        cause: error,
      },
    );
  }

  return importWasmBindings();
}

async function importWasmBindings() {
  const nextPath = "@takumi-rs/wasm/next";
  if (typeof process !== "undefined" && process.env.NEXT_RUNTIME) {
    return import(/* @__PURE__ */ /* @vite-ignore */ nextPath) as Promise<
      typeof import("@takumi-rs/wasm/next")
    >;
  }

  return import(
    /* @__PURE__ */ /* turbopackIgnore: true */ /* webpackIgnore: true */ "@takumi-rs/wasm/auto"
  ) as Promise<typeof import("@takumi-rs/wasm/auto")>;
}

function shouldSkipCoreImport() {
  if (typeof process !== "undefined" && process.env.NEXT_RUNTIME === "edge") {
    return true;
  }

  if (typeof window !== "undefined") {
    return true;
  }

  if (typeof navigator !== "undefined" && navigator.userAgent === "Cloudflare-Workers") {
    return true;
  }

  // Cloudflare Workers runtime provides this global.
  if ("WebSocketPair" in globalThis) {
    return true;
  }

  if ("EdgeRuntime" in globalThis) {
    return true;
  }

  const maybeWorkerGlobalScope = (
    globalThis as typeof globalThis & {
      WorkerGlobalScope?: { prototype: object };
    }
  ).WorkerGlobalScope;
  if (
    maybeWorkerGlobalScope !== undefined &&
    maybeWorkerGlobalScope.prototype.isPrototypeOf(globalThis)
  ) {
    return true;
  }

  return false;
}
