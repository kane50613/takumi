import type { CompletionContext } from "@codemirror/autocomplete";
import {
  createSystem,
  createVirtualTypeScriptEnvironment,
  type VirtualTypeScriptEnvironment,
} from "@typescript/vfs";
import { getAutocompletion, getHover, getLints } from "@valtown/codemirror-ts";
import * as Comlink from "comlink";
// The root `typescript` is the Go port, which has no browser language service.
import ts from "typescript-5";

const libraryTypings = import.meta.glob<string>(
  "../../../node_modules/typescript-5/lib/lib.*.d.ts",
  { query: "?raw", import: "default", eager: true },
);

// The same augmentation `takumi-js` ships, which the snippet never imports.
const tailwindTypings = `
import "react";

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}
`;

const compilerOptions: ts.CompilerOptions = {
  target: ts.ScriptTarget.Latest,
  module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Bundler,
  jsx: ts.JsxEmit.ReactJSX,
  esModuleInterop: true,
  allowNonTsExtensions: true,
  baseUrl: "/",
  paths: {
    "echarts/*": ["/node_modules/echarts/types/dist/*"],
  },
};

const files = new Map(
  Object.entries(libraryTypings).map(([path, content]) => [
    `/${path.slice(path.lastIndexOf("/") + 1)}`,
    content,
  ]),
);

let environment: VirtualTypeScriptEnvironment | undefined;

function ensureEnvironment(): VirtualTypeScriptEnvironment {
  environment ??= createVirtualTypeScriptEnvironment(createSystem(files), [], ts, compilerOptions);

  return environment;
}

/** Keeps the promise only while it is pending or fulfilled, so a failed load is retried. */
function retryable(load: () => Promise<void>) {
  let pending: Promise<void> | undefined;

  return () => {
    pending ??= load().catch((error: unknown) => {
      pending = undefined;
      throw error;
    });

    return pending;
  };
}

function raw(module: { default: string }) {
  return module.default;
}

const loadCoreTypings = retryable(async () => {
  const [react, reactJsxRuntime, csstype, takumi, pdfPrimitives, playgroundOptions] =
    await Promise.all([
      import("../../../node_modules/@types/react/index.d.ts?raw").then(raw),
      import("../../../node_modules/@types/react/jsx-runtime.d.ts?raw").then(raw),
      import("../../../node_modules/csstype/index.d.ts?raw").then(raw),
      import("../../../node_modules/@takumi-rs/wasm/pkg/takumi_wasm_bg.wasm.d.ts?raw").then(raw),
      import("../../../node_modules/takumi-pdf/dist/primitives.d.mts?raw").then(raw),
      import("../../playground/options.ts?raw").then(raw),
    ]);

  files.set("/node_modules/react/index.d.ts", react);
  files.set("/node_modules/react/jsx-runtime.d.ts", reactJsxRuntime);
  files.set("/node_modules/csstype/index.d.ts", csstype);
  files.set("/node_modules/@takumi-rs/wasm/index.d.ts", takumi);
  files.set("/node_modules/takumi-pdf/primitives.d.ts", pdfPrimitives);
  // Ambient declarations only reach the program as root files.
  ensureEnvironment().createFile("/options.d.ts", playgroundOptions);
  ensureEnvironment().createFile("/tw.d.ts", tailwindTypings);
});

const loadEchartsTypings = retryable(async () => {
  const modules = await Promise.all([
    import("../../../node_modules/echarts/types/dist/core.d.ts?raw").then(raw),
    import("../../../node_modules/echarts/types/dist/charts.d.ts?raw").then(raw),
    import("../../../node_modules/echarts/types/dist/components.d.ts?raw").then(raw),
    import("../../../node_modules/echarts/types/dist/renderers.d.ts?raw").then(raw),
    import("../../../node_modules/echarts/types/dist/shared.d.ts?raw").then(raw),
  ]);
  const names = ["core", "charts", "components", "renderers", "shared"];

  modules.forEach((content, index) => {
    files.set(`/node_modules/echarts/types/dist/${names[index]}.d.ts`, content);
  });
});

export type SignatureHelp = {
  prefix: string;
  parameters: string[];
  separator: string;
  suffix: string;
  argumentIndex: number;
};

const worker = {
  initialize() {
    ensureEnvironment();
  },
  loadTypings() {
    return loadCoreTypings();
  },
  loadEchartsTypings() {
    return loadEchartsTypings();
  },
  updateFile({ path, code }: { path: string; code: string }) {
    const env = ensureEnvironment();

    if (env.getSourceFile(path)) {
      env.updateFile(path, code);
      return;
    }

    env.createFile(path, code);
  },
  getLints({ path, diagnosticCodesToIgnore }: { path: string; diagnosticCodesToIgnore: number[] }) {
    return getLints({ env: ensureEnvironment(), path, diagnosticCodesToIgnore });
  },
  getAutocompletion({
    path,
    context,
  }: {
    path: string;
    context: Pick<CompletionContext, "pos" | "explicit">;
  }) {
    return getAutocompletion({ env: ensureEnvironment(), path, context });
  },
  getHover({ path, pos }: { path: string; pos: number }) {
    return getHover({ env: ensureEnvironment(), path, pos });
  },
  /** Part of codemirror-ts's `WorkerShape`; its extensions never call it. */
  getEnv() {
    return ensureEnvironment();
  },
  getSignatureHelp({ path, pos }: { path: string; pos: number }): SignatureHelp | undefined {
    const help = ensureEnvironment().languageService.getSignatureHelpItems(path, pos, {});
    const item = help?.items[help.selectedItemIndex];

    if (!help || !item) return undefined;

    const text = (parts: ts.SymbolDisplayPart[]) => parts.map((part) => part.text).join("");

    return {
      prefix: text(item.prefixDisplayParts),
      parameters: item.parameters.map((parameter) => text(parameter.displayParts)),
      separator: text(item.separatorDisplayParts),
      suffix: text(item.suffixDisplayParts),
      argumentIndex: help.argumentIndex,
    };
  },
};

export type TypeScriptWorker = typeof worker;

Comlink.expose(worker);
