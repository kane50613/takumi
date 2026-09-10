"use client";

import { Editor } from "@monaco-editor/react";
import type { Monaco } from "@monaco-editor/react";
import { useTheme } from "next-themes";
import { useEffect, useRef, useState } from "react";
import type { ComponentProps } from "react";
import { darkTheme, lightTheme, registerSyntaxHighlighting } from "./syntax-highlighting";

type ExtraLib = { content: string; filePath: string };

const extraLib = (filePath: string) => (module: { default: string }) => ({
  content: module.default,
  filePath,
});

// Dynamic imports keep the typings out of the playground chunk; they load as
// their own chunks alongside the editor.
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
  Promise.all([
    import("../../../node_modules/echarts/types/dist/core.d.ts?raw").then(
      extraLib("file:///node_modules/echarts/types/dist/core.d.ts"),
    ),
    import("../../../node_modules/echarts/types/dist/charts.d.ts?raw").then(
      extraLib("file:///node_modules/echarts/types/dist/charts.d.ts"),
    ),
    import("../../../node_modules/echarts/types/dist/components.d.ts?raw").then(
      extraLib("file:///node_modules/echarts/types/dist/components.d.ts"),
    ),
    import("../../../node_modules/echarts/types/dist/renderers.d.ts?raw").then(
      extraLib("file:///node_modules/echarts/types/dist/renderers.d.ts"),
    ),
    import("../../../node_modules/echarts/types/dist/shared.d.ts?raw").then(
      extraLib("file:///node_modules/echarts/types/dist/shared.d.ts"),
    ),
  ]);

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
function applyTypings(monaco: Monaco) {
  monaco.languages.typescript.typescriptDefaults.setExtraLibs([tailwindTypings, ...loadedTypings]);
}

/** Shares one load across mounts; a failed load is dropped so the next mount retries it. */
function typingsLoader(typings: () => Promise<ExtraLib[]>) {
  let load: Promise<void> | undefined;

  return (monaco: Monaco) =>
    (load ??= typings()
      .then((libs) => {
        loadedTypings.push(...libs);
        applyTypings(monaco);
      })
      .catch((error: unknown) => {
        load = undefined;
        throw error;
      }));
}

const loadCoreTypings = typingsLoader(coreTypings);
const loadEchartsTypings = typingsLoader(echartsTypings);

export function ComponentEditor({
  code,
  setCode,
  onRun,
}: {
  code: string;
  setCode: (code: string) => void;
  onRun: () => void;
}) {
  const { resolvedTheme } = useTheme();
  const [isMobileViewport, setIsMobileViewport] = useState(false);
  const editorRef = useRef<
    Parameters<NonNullable<ComponentProps<typeof Editor>["onMount"]>>[0] | null
  >(null);
  const isApplyingExternalCodeRef = useRef(false);
  // The command is registered once, so it reads the callback through a ref.
  const onRunRef = useRef(onRun);

  onRunRef.current = onRun;
  /** The last value the editor itself produced, so its own edits never bounce back. */
  const lastEmittedRef = useRef(code);
  const theme = resolvedTheme === "dark" ? darkTheme : lightTheme;

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const mobileMediaQuery = window.matchMedia("(max-width: 640px)");
    const updateMobileViewport = () => setIsMobileViewport(mobileMediaQuery.matches);

    updateMobileViewport();
    mobileMediaQuery.addEventListener("change", updateMobileViewport);

    return () => {
      mobileMediaQuery.removeEventListener("change", updateMobileViewport);
    };
  }, []);

  // Only a change from outside the editor (a template, a reset, formatting) is
  // worth writing back. An IME composes through several intermediate values, and
  // the state lags behind them, so comparing against the model alone would make
  // every stale round trip look external.
  useEffect(() => {
    const editor = editorRef.current;
    const model = editor?.getModel();

    if (!editor || !model || code === lastEmittedRef.current || model.getValue() === code) {
      return;
    }

    const selection = editor.getSelection();

    isApplyingExternalCodeRef.current = true;
    // A full-range edit rather than `setValue`: it keeps the undo stack and
    // leaves the cursor where it was instead of dropping it at the top.
    model.pushEditOperations([], [{ range: model.getFullModelRange(), text: code }], () => null);
    isApplyingExternalCodeRef.current = false;
    lastEmittedRef.current = code;

    if (selection) {
      editor.setSelection(selection);
    }
  }, [code]);

  return (
    <Editor
      beforeMount={(monaco) => {
        monaco.languages.typescript.typescriptDefaults.setCompilerOptions({
          target: monaco.languages.typescript.ScriptTarget.Latest,
          allowNonTsExtensions: true,
          moduleResolution: monaco.languages.typescript.ModuleResolutionKind.NodeJs,
          module: monaco.languages.typescript.ModuleKind.ESNext,
          reactNamespace: "React",
          esModuleInterop: true,
          jsx: monaco.languages.typescript.JsxEmit.ReactJSX,
          typeRoots: ["node_modules/@types"],
          baseUrl: "file:///",
          paths: {
            "echarts/*": ["node_modules/echarts/types/dist/*"],
          },
        });

        applyTypings(monaco);
        registerSyntaxHighlighting(monaco);
      }}
      onMount={(editor, monaco) => {
        editorRef.current = editor;
        // Monaco owns the keyboard inside the editor and binds ⌘↵ itself, so the
        // shortcut has to be registered here rather than on the window.
        editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => onRunRef.current());

        loadCoreTypings(monaco);

        // echarts ships megabytes of typings, so they wait until the code asks for them.
        const loadEchartsTypingsIfUsed = () => {
          if (editor.getModel()?.getValue().includes("echarts")) {
            loadEchartsTypings(monaco);
          }
        };

        loadEchartsTypingsIfUsed();
        editor.onDidChangeModelContent(loadEchartsTypingsIfUsed);
      }}
      width="100%"
      height="100%"
      language="typescript"
      theme={theme}
      path="main.tsx"
      options={{
        automaticLayout: true,
        wordWrap: "on",
        tabSize: 2,
        minimap: {
          enabled: false,
        },
        glyphMargin: !isMobileViewport,
        folding: !isMobileViewport,
        stickyScroll: {
          enabled: false,
        },
        scrollbar: {
          useShadows: false,
          verticalScrollbarSize: isMobileViewport ? 8 : 10,
          horizontalScrollbarSize: isMobileViewport ? 8 : 10,
        },
        lineNumbers: isMobileViewport ? "off" : "on",
        lineDecorationsWidth: isMobileViewport ? 8 : 10,
        lineNumbersMinChars: isMobileViewport ? 0 : 3,
        overviewRulerLanes: isMobileViewport ? 0 : 2,
        fontSize: isMobileViewport ? 13 : 16,
        padding: {
          top: isMobileViewport ? 6 : 8,
          bottom: isMobileViewport ? 6 : 8,
        },
        scrollBeyondLastLine: false,
      }}
      loading="Launching editor..."
      defaultValue={code}
      onChange={(value) => {
        if (value !== undefined && !isApplyingExternalCodeRef.current) {
          lastEmittedRef.current = value;
          setCode(value);
        }
      }}
    />
  );
}
