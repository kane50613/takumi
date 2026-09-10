import type { editor } from "monaco-editor/esm/vs/editor/editor.api.js";
import {
  JsxEmit,
  ModuleKind,
  ModuleResolutionKind,
  ScriptTarget,
  typescriptDefaults,
} from "monaco-editor/esm/vs/language/typescript/monaco.contribution.js";
import { setupTypeScript } from "monaco-editor/esm/vs/language/typescript/tsMode.js";

type ExtraLib = { content: string; filePath: string };

const extraLib = (filePath: string) => (module: { default: string }) => ({
  content: module.default,
  filePath,
});

const coreTypings = () =>
  Promise.all([
    import("../../../node_modules/@types/react/index.d.ts?raw").then(
      extraLib("file:///node_modules/react/index.d.ts"),
    ),
    import("../../../node_modules/@types/react/jsx-runtime.d.ts?raw").then(
      extraLib("file:///node_modules/react/jsx-runtime.d.ts"),
    ),
    import("../../../node_modules/csstype/index.d.ts?raw").then(
      extraLib("file:///node_modules/csstype/index.d.ts"),
    ),
    import("../../../node_modules/@takumi-rs/wasm/pkg/takumi_wasm_bg.wasm.d.ts?raw").then(
      extraLib("file:///node_modules/@takumi-rs/wasm/index.d.ts"),
    ),
    import("../../../node_modules/takumi-pdf/dist/primitives.d.mts?raw").then(
      extraLib("file:///node_modules/takumi-pdf/primitives.d.ts"),
    ),
    import("../../playground/options.ts?raw").then(extraLib("file:///options.d.ts")),
  ]);

const echartsTypings = () =>
  Promise.all(
    Object.entries(
      // Vite skips node_modules unless the glob is exhaustive.
      import.meta.glob<string>(
        "../../../node_modules/echarts/types/dist/{core,charts,components,renderers,shared}.d.ts",
        { query: "?raw", import: "default", exhaustive: true },
      ),
    ).map(async ([path, load]) => ({
      content: await load(),
      filePath: `file:///${path.slice(path.indexOf("node_modules"))}`,
    })),
  );

const tailwindTypings: ExtraLib = {
  content: `
declare namespace React {
  interface HTMLAttributes<T> {
    tw?: string;
  }
}
`,
  filePath: "file:///tw.d.ts",
};

const loadedTypings: ExtraLib[] = [];

/** Monaco replaces the whole set, so every mount rewrites it from what has loaded so far. */
function applyTypings() {
  typescriptDefaults.setExtraLibs([tailwindTypings, ...loadedTypings]);
}

/** A failed load is dropped so the next mount retries it. */
function typingsLoader(typings: () => Promise<ExtraLib[]>) {
  let load: Promise<void> | undefined;

  return () =>
    (load ??= typings()
      .then((libs) => {
        loadedTypings.push(...libs);
        applyTypings();
      })
      .catch((error: unknown) => {
        load = undefined;
        throw error;
      }));
}

const loadCoreTypings = typingsLoader(coreTypings);
const loadEchartsTypings = typingsLoader(echartsTypings);

export function startTypeScriptService(codeEditor: editor.IStandaloneCodeEditor) {
  typescriptDefaults.setCompilerOptions({
    target: ScriptTarget.Latest,
    allowNonTsExtensions: true,
    moduleResolution: ModuleResolutionKind.NodeJs,
    module: ModuleKind.ESNext,
    reactNamespace: "React",
    esModuleInterop: true,
    jsx: JsxEmit.ReactJSX,
    typeRoots: ["node_modules/@types"],
    baseUrl: "file:///",
    paths: {
      "echarts/*": ["node_modules/echarts/types/dist/*"],
    },
  });

  applyTypings();
  // The contribution hooks `onLanguage`, which fired when the editor created its model.
  setupTypeScript(typescriptDefaults);
  loadCoreTypings();

  // echarts ships megabytes of typings, so they wait until the code asks for them.
  const loadEchartsTypingsIfUsed = () => {
    if (codeEditor.getModel()?.getValue().includes("echarts")) {
      loadEchartsTypings();
    }
  };

  loadEchartsTypingsIfUsed();
  codeEditor.onDidChangeModelContent(loadEchartsTypingsIfUsed);
}
