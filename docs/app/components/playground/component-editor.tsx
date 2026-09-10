"use client";

import { Editor } from "@monaco-editor/react";
import { useTheme } from "next-themes";
import { useEffect, useRef, useState } from "react";
import type { ComponentProps } from "react";
import type { startTypeScriptService } from "./monaco";
import { darkTheme, lightTheme } from "./syntax-highlighting";

type MonacoModule = { startTypeScriptService: typeof startTypeScriptService };

let monacoModule: Promise<MonacoModule> | undefined;

/** monaco-editor reaches for `document` while it evaluates, so it may only load in the browser. */
const loadMonaco = () => (monacoModule ??= import("./monaco"));

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
  const [monaco, setMonaco] = useState<MonacoModule | null>(null);
  const [isMobileViewport, setIsMobileViewport] = useState(false);
  const editorRef = useRef<
    Parameters<NonNullable<ComponentProps<typeof Editor>["onMount"]>>[0] | null
  >(null);
  const isApplyingExternalCodeRef = useRef(false);
  /** The command is registered once, so it reads the callback through a ref. */
  const onRunRef = useRef(onRun);
  /** The last value the editor itself produced, so its own edits never bounce back. */
  const lastEmittedRef = useRef(code);

  onRunRef.current = onRun;

  const theme = resolvedTheme === "dark" ? darkTheme : lightTheme;

  useEffect(() => {
    let isMounted = true;

    loadMonaco().then((module) => {
      if (isMounted) {
        setMonaco(module);
      }
    });

    return () => {
      isMounted = false;
    };
  }, []);

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

  // An IME lags the state behind the model, so only the last emitted value tells an outside edit apart.
  useEffect(() => {
    const editor = editorRef.current;
    const model = editor?.getModel();

    if (!editor || !model || code === lastEmittedRef.current || model.getValue() === code) {
      return;
    }

    const selection = editor.getSelection();

    isApplyingExternalCodeRef.current = true;
    // A full-range edit keeps the undo stack, which `setValue` drops.
    model.pushEditOperations([], [{ range: model.getFullModelRange(), text: code }], () => null);
    isApplyingExternalCodeRef.current = false;
    lastEmittedRef.current = code;

    if (selection) {
      editor.setSelection(selection);
    }
  }, [code]);

  if (!monaco) {
    return (
      <div className="grid h-full w-full place-items-center text-fd-muted-foreground text-sm">
        Launching editor...
      </div>
    );
  }

  return (
    <Editor
      onMount={(editor, { KeyCode, KeyMod }) => {
        editorRef.current = editor;
        // Monaco owns the keyboard inside the editor, so ⌘↵ cannot be bound on the window.
        editor.addCommand(KeyMod.CtrlCmd | KeyCode.Enter, () => onRunRef.current());

        monaco.startTypeScriptService(editor);
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
