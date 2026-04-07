import type * as WasmBindings from "@takumi-rs/wasm";
import type * as WasmAutoBindings from "@takumi-rs/wasm/auto";
import type * as WasmNextBindings from "@takumi-rs/wasm/next";

export type Imports = Awaited<ReturnType<typeof getImportsImpl>>;

let importPromise: Promise<Imports> | null = null;

export function getImports(module?: WasmBindings.InitInput) {
  importPromise ??= getImportsImpl(module);

  return importPromise;
}

async function getImportsImpl(module?: WasmBindings.InitInput) {
  if (module) {
    return initializeWasm(module);
  }

  if (shouldSkipCoreImport()) {
    return initializeWasm(importWasmBindings());
  }

  try {
    return await import("@takumi-rs/core");
  } catch (error) {
    if (isNodeEnvironment()) {
      throw new Error(
        "Failed to load @takumi-rs/core in Node.js runtime. Takumi requires the native napi-rs module in Node environments.",
        { cause: error },
      );
    }

    console.warn(
      "Unable to import @takumi-rs/core. Falling back to auto-detection of WASM bindings.",
      {
        cause: error,
      },
    );
  }

  return initializeWasm(importWasmBindings());
}

type WasmModuleInput =
  | WasmBindings.InitInput
  | { default: WasmBindings.InitInput }
  | Promise<WasmBindings.InitInput | { default: WasmBindings.InitInput }>
  | (() => Promise<WasmBindings.InitInput | { default: WasmBindings.InitInput }>);

async function initializeWasm(module?: WasmModuleInput) {
  const wasmPath = "@takumi-rs/wasm";
  const wasm = (await import(/* @vite-ignore */ wasmPath)) as typeof WasmBindings;
  const resolvedModule = typeof module === "function" ? await module() : await module;
  const wasmModule =
    resolvedModule !== undefined &&
    typeof resolvedModule === "object" &&
    "default" in resolvedModule
      ? resolvedModule.default
      : resolvedModule;

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

function hackFakeProcessForBrowser() {
  const before = globalThis.process;

  // @ts-expect-error: In order to keep the NEXT_RUNTIME check statically analyzable, no other checks can be added before it.
  // But it will break Cloudflare Workers runtime which doesn't have process at all, so we need to hack a fake process for browser environments.
  globalThis.process ??= {};
  globalThis.process.env ??= {};

  return before;
}

async function importWasmBindings() {
  const beforeProcess = hackFakeProcessForBrowser();

  const nextPath = "@takumi-rs/wasm/next";
  if (process.env.NEXT_RUNTIME) {
    globalThis.process = beforeProcess;
    return import(/* @vite-ignore */ nextPath) as Promise<typeof WasmNextBindings>;
  }

  globalThis.process = beforeProcess;

  return import(
    /* turbopackIgnore: true */ /* webpackIgnore: true */ "@takumi-rs/wasm/auto"
  ) as Promise<typeof WasmAutoBindings>;
}

function shouldSkipCoreImport() {
  const beforeProcess = hackFakeProcessForBrowser();

  if (process.env.NEXT_RUNTIME === "edge") {
    globalThis.process = beforeProcess;

    return true;
  }

  globalThis.process = beforeProcess;

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

function isNodeEnvironment() {
  return (
    typeof process !== "undefined" &&
    typeof process.versions === "object" &&
    process.versions !== null &&
    typeof process.versions.node === "string"
  );
}
