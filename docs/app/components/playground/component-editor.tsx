"use client";

import {
  autocompletion,
  closeBrackets,
  closeBracketsKeymap,
  completionKeymap,
} from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { javascript } from "@codemirror/lang-javascript";
import {
  bracketMatching,
  foldGutter,
  foldKeymap,
  indentOnInput,
  indentUnit,
} from "@codemirror/language";
import { lintKeymap } from "@codemirror/lint";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  drawSelection,
  dropCursor,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  rectangularSelection,
} from "@codemirror/view";
import githubDarkDefault from "@shikijs/themes/github-dark-default";
import githubLightDefault from "@shikijs/themes/github-light-default";
import { useTheme } from "next-themes";
import { useEffect, useRef, useState } from "react";
import { editorTheme } from "./editor-theme";
import { startTypeScriptService, type TypeScriptService } from "./typescript-service";

const THEMES = {
  dark: editorTheme(githubDarkDefault),
  light: editorTheme(githubLightDefault),
};

/** Safari has no `requestIdleCallback`, so a timer stands in for it there. */
function whenIdle(task: () => void) {
  if (typeof requestIdleCallback === "function") {
    const handle = requestIdleCallback(task);

    return () => cancelIdleCallback(handle);
  }

  const handle = setTimeout(task, 500);

  return () => clearTimeout(handle);
}

/** Line numbers and folding cost more than they are worth on a phone. */
function viewportExtensions(isMobileViewport: boolean): Extension {
  return [
    EditorView.theme({
      "&": { fontSize: isMobileViewport ? "13px" : "16px" },
      ".cm-content": { padding: isMobileViewport ? "6px 0" : "8px 0" },
    }),
    isMobileViewport ? [] : [lineNumbers(), highlightActiveLineGutter(), foldGutter()],
  ];
}

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
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView>(null);
  // The editor is built once, so its extensions read the callbacks through refs.
  const onRunRef = useRef(onRun);
  const setCodeRef = useRef(setCode);
  /** The last value the editor itself produced, so its own edits never bounce back. */
  const lastEmittedRef = useRef(code);
  const [compartments] = useState(() => ({
    theme: new Compartment(),
    viewport: new Compartment(),
    typescript: new Compartment(),
  }));

  onRunRef.current = onRun;
  setCodeRef.current = setCode;

  useEffect(() => {
    const mobileMediaQuery = window.matchMedia("(max-width: 640px)");
    const updateMobileViewport = () => setIsMobileViewport(mobileMediaQuery.matches);

    updateMobileViewport();
    mobileMediaQuery.addEventListener("change", updateMobileViewport);

    return () => {
      mobileMediaQuery.removeEventListener("change", updateMobileViewport);
    };
  }, []);

  useEffect(() => {
    const view = new EditorView({
      doc: lastEmittedRef.current,
      parent: containerRef.current ?? undefined,
      extensions: [
        keymap.of([
          {
            key: "Mod-Enter",
            run: () => {
              onRunRef.current();
              return true;
            },
          },
          indentWithTab,
          ...closeBracketsKeymap,
          ...completionKeymap,
          ...defaultKeymap,
          ...searchKeymap,
          ...historyKeymap,
          ...foldKeymap,
          ...lintKeymap,
        ]),
        javascript({ jsx: true, typescript: true }),
        history(),
        drawSelection(),
        dropCursor(),
        rectangularSelection(),
        highlightSpecialChars(),
        highlightActiveLine(),
        highlightSelectionMatches(),
        indentOnInput(),
        bracketMatching(),
        closeBrackets(),
        EditorState.tabSize.of(2),
        indentUnit.of("  "),
        EditorView.lineWrapping,
        EditorView.theme({
          "&": { height: "100%", fontFamily: "var(--font-mono)" },
          ".cm-scroller": { fontFamily: "inherit", lineHeight: "1.5" },
        }),
        EditorView.updateListener.of((update) => {
          if (!update.docChanged) return;

          const value = update.state.doc.toString();

          lastEmittedRef.current = value;
          setCodeRef.current(value);
        }),
        compartments.theme.of(THEMES.dark),
        compartments.viewport.of(viewportExtensions(false)),
        compartments.typescript.of(autocompletion()),
      ],
    });

    viewRef.current = view;

    return () => {
      viewRef.current = null;
      view.destroy();
    };
  }, [compartments]);

  useEffect(() => {
    viewRef.current?.dispatch({
      effects: compartments.theme.reconfigure(
        resolvedTheme === "dark" ? THEMES.dark : THEMES.light,
      ),
    });
  }, [resolvedTheme, compartments]);

  useEffect(() => {
    viewRef.current?.dispatch({
      effects: compartments.viewport.reconfigure(viewportExtensions(isMobileViewport)),
    });
  }, [isMobileViewport, compartments]);

  // The language service is the heaviest part of the editor, so it starts once
  // the browser is otherwise idle and joins the editor that is already running.
  useEffect(() => {
    let service: TypeScriptService | undefined;
    let disposed = false;
    const cancel = whenIdle(() => {
      startTypeScriptService()
        .then((started) => {
          service = started;

          if (disposed) return started.dispose();

          viewRef.current?.dispatch({
            effects: compartments.typescript.reconfigure(started.extensions),
          });
        })
        .catch((error: unknown) => console.error("Failed to start the language service:", error));
    });

    return () => {
      disposed = true;
      cancel();
      service?.dispose();
    };
  }, [compartments]);

  // Only a change from outside the editor (a template, a share link, formatting)
  // is worth writing back, and it goes in as one transaction so undo still walks
  // back through everything typed before it.
  useEffect(() => {
    const view = viewRef.current;

    if (!view || code === lastEmittedRef.current) return;

    const current = view.state.doc.toString();

    if (current === code) return;

    lastEmittedRef.current = code;
    view.dispatch({ changes: { from: 0, to: current.length, insert: code } });
  }, [code]);

  return <div ref={containerRef} className="h-full w-full overflow-hidden" />;
}
