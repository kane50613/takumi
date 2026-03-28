import { Editor } from "@monaco-editor/react";
import { shikiToMonaco } from "@shikijs/monaco";
import { useTheme } from "next-themes";
import { useEffect, useRef, useState } from "react";
import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine-oniguruma.mjs";
import takumiTypings from "../../../node_modules/@takumi-rs/wasm/pkg/takumi_wasm_bg.wasm.d.ts?raw";
import reactTypings from "../../../node_modules/@types/react/index.d.ts?raw";
import reactJsxRuntimeTypings from "../../../node_modules/@types/react/jsx-runtime.d.ts?raw";
import cssTypings from "../../../node_modules/csstype/index.d.ts?raw";
import playgroundOptionsTypings from "../../playground/options.ts?raw";

function createHighlighter() {
  return createHighlighterCore({
    themes: [
      import("shiki/themes/github-dark-default.mjs"),
      import("shiki/themes/github-light-default.mjs"),
    ],
    langs: [import("shiki/langs/tsx.mjs")],
    engine: createOnigurumaEngine(import("shiki/wasm")),
    langAlias: {
      typescript: "tsx",
    },
  });
}

type GlobalThis = typeof globalThis & {
  shikiInstance: ReturnType<typeof createHighlighter>;
};

(globalThis as GlobalThis).shikiInstance ??= createHighlighter();

const highlighter = await (globalThis as GlobalThis).shikiInstance;

const tailwindTypings = `
declare namespace React {
  interface HTMLAttributes<T> {
    tw?: string;
  }
}
`;

export function ComponentEditor({
  code,
  setCode,
}: {
  code: string;
  setCode: (code: string) => void;
}) {
  const { resolvedTheme } = useTheme();
  const [isMobileViewport, setIsMobileViewport] = useState(false);
  const editorRef = useRef<
    Parameters<NonNullable<React.ComponentProps<typeof Editor>["onMount"]>>[0] | null
  >(null);
  const isApplyingExternalCodeRef = useRef(false);
  const theme = resolvedTheme === "dark" ? "github-dark-default" : "github-light-default";

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

  useEffect(() => {
    const editor = editorRef.current;
    const model = editor?.getModel();

    if (!editor || !model || model.getValue() === code) {
      return;
    }

    isApplyingExternalCodeRef.current = true;
    editor.setValue(code);
    isApplyingExternalCodeRef.current = false;
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
        });

        monaco.languages.typescript.typescriptDefaults.setExtraLibs([
          {
            content: reactTypings,
            filePath: "file:///node_modules/react/index.d.ts",
          },
          {
            content: reactJsxRuntimeTypings,
            filePath: "file:///node_modules/react/jsx-runtime.d.ts",
          },
          {
            content: cssTypings,
            filePath: "file:///node_modules/csstype/index.d.ts",
          },
          {
            content: takumiTypings,
            filePath: "file:///node_modules/@takumi-rs/wasm/index.d.ts",
          },
          {
            content: playgroundOptionsTypings,
            filePath: "file:///options.d.ts",
          },
          {
            content: tailwindTypings,
            filePath: "file:///tw.d.ts",
          },
        ]);

        shikiToMonaco(highlighter, monaco);
      }}
      onMount={(editor) => {
        editorRef.current = editor;
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
          setCode(value);
        }
      }}
    />
  );
}
