"use client";

import { autocompletion, closeBrackets } from "@codemirror/autocomplete";
import { history } from "@codemirror/commands";
import { javascript } from "@codemirror/lang-javascript";
import { bracketMatching, foldGutter, indentOnInput, indentUnit } from "@codemirror/language";
import { highlightSelectionMatches, search } from "@codemirror/search";
import { Compartment, EditorState, type Extension, RangeSetBuilder } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  drawSelection,
  dropCursor,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  rectangularSelection,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";
import githubDarkDefault from "@shikijs/themes/github-dark-default";
import githubLightDefault from "@shikijs/themes/github-light-default";
import { useTheme } from "next-themes";
import { useEffect, useRef, useState } from "react";
import { editorBindings, type PlaygroundActions } from "./editor-commands";
import { editorTheme } from "./editor-theme";
import { FindWidget, findWidgetTheme } from "./find-widget";
import { textmateScopes } from "./textmate-scopes";
import { startTypeScriptService, type TypeScriptService } from "./typescript-service";

const THEMES = {
  dark: editorTheme(githubDarkDefault),
  light: editorTheme(githubLightDefault),
};

function indentDecorations(view: EditorView) {
  const builder = new RangeSetBuilder<Decoration>();

  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to;) {
      const line = view.state.doc.lineAt(pos);
      const indent = line.text.length - line.text.trimStart().length;

      if (indent > 0) {
        builder.add(
          line.from,
          line.from,
          Decoration.line({
            attributes: {
              style: `padding-left: calc(6px + ${indent}ch); text-indent: -${indent}ch`,
            },
          }),
        );
      }

      pos = line.to + 1;
    }
  }

  return builder.finish();
}

/** Hangs a wrapped line under its own indent, the way Monaco wrapped one. */
const indentedWrapping = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = indentDecorations(view);
    }

    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = indentDecorations(update.view);
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

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
  onFormat,
}: {
  code: string;
  setCode: (code: string) => void;
  onRun: () => void;
  onFormat: () => void;
}) {
  const { resolvedTheme } = useTheme();
  const [isMobileViewport, setIsMobileViewport] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView>(null);
  // The editor is built once, so its extensions read the callbacks through refs.
  const onRunRef = useRef(onRun);
  const onFormatRef = useRef(onFormat);
  const setCodeRef = useRef(setCode);
  /** The last value the editor itself produced, so its own edits never bounce back. */
  const lastEmittedRef = useRef(code);
  const [compartments] = useState(() => ({
    theme: new Compartment(),
    viewport: new Compartment(),
    typescript: new Compartment(),
  }));

  onRunRef.current = onRun;
  onFormatRef.current = onFormat;
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
    const actions: PlaygroundActions = {
      run: () => {
        onRunRef.current();
        return true;
      },
      format: () => {
        onFormatRef.current();
        return true;
      },
    };
    const view = new EditorView({
      doc: lastEmittedRef.current,
      parent: containerRef.current ?? undefined,
      extensions: [
        keymap.of([...editorBindings(actions)]),
        search({ top: true, createPanel: (view) => new FindWidget(view) }),
        findWidgetTheme,
        javascript({ jsx: true, typescript: true }),
        textmateScopes,
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
        EditorState.allowMultipleSelections.of(true),
        EditorState.tabSize.of(2),
        indentUnit.of("  "),
        EditorView.lineWrapping,
        indentedWrapping,
        EditorView.theme({
          "&": { height: "100%", fontFamily: "var(--font-mono)" },
          ".cm-foldGutter .cm-gutterElement": { opacity: 0, transition: "opacity 120ms" },
          "&:hover .cm-foldGutter .cm-gutterElement, .cm-foldGutter [title='Unfold line']": {
            opacity: 1,
          },
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
