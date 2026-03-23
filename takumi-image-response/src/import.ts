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

async function importBindings() {
  // Next.js path
  const nextPath = "@takumi-rs/wasm/next";
  if (typeof process !== "undefined" && process.env.NEXT_RUNTIME === "edge") {
    return import(/* @__PURE__ */ /* @vite-ignore */ nextPath) as Promise<
      typeof import("@takumi-rs/wasm/next")
    >;
  }

  try {
    return await import(
      /* @__PURE__ */ /* turbopackIgnore: true */ /* webpackIgnore: true */ "@takumi-rs/core/auto"
    );
  } catch {
    if (typeof process !== "undefined" && process.env.NEXT_RUNTIME) {
      return import(/* @__PURE__ */ /* @vite-ignore */ nextPath) as Promise<
        typeof import("@takumi-rs/wasm/next")
      >;
    }

    return import(
      /* @__PURE__ */ /* turbopackIgnore: true */ /* webpackIgnore: true */ "@takumi-rs/wasm/auto"
    ) as Promise<typeof import("@takumi-rs/wasm/auto")>;
  }
}
