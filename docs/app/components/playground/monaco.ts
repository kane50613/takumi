import { loader } from "@monaco-editor/react";
import { shikiToMonaco } from "@shikijs/monaco";
import { conf as typescriptConfiguration } from "monaco-editor/languages/definitions/typescript/typescript.js";
import "monaco-editor/features/codicon/register.js";
import "monaco-editor/editor/browser/coreCommands.js";
import "monaco-editor/editor/browser/widget/codeEditor/codeEditorWidget.js";
import "monaco-editor/editor/contrib/bracketMatching/browser/bracketMatching.js";
import "monaco-editor/editor/contrib/clipboard/browser/clipboard.js";
import "monaco-editor/editor/contrib/comment/browser/comment.js";
import "monaco-editor/editor/contrib/contextmenu/browser/contextmenu.js";
import "monaco-editor/editor/contrib/find/browser/findController.js";
import "monaco-editor/editor/contrib/folding/browser/folding.js";
import "monaco-editor/editor/contrib/format/browser/formatActions.js";
import "monaco-editor/editor/contrib/hover/browser/hoverContribution.js";
import "monaco-editor/editor/contrib/indentation/browser/indentation.js";
import "monaco-editor/editor/contrib/lineSelection/browser/lineSelection.js";
import "monaco-editor/editor/contrib/linesOperations/browser/linesOperations.js";
import "monaco-editor/editor/contrib/parameterHints/browser/parameterHints.js";
import "monaco-editor/editor/contrib/suggest/browser/suggestController.js";
import "monaco-editor/editor/contrib/wordHighlighter/browser/wordHighlighter.js";
import "monaco-editor/editor/contrib/wordOperations/browser/wordOperations.js";
import * as monaco from "monaco-editor/editor/editor.api.js";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import TypeScriptWorker from "monaco-editor/language/typescript/ts.worker.js?worker";
import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine-oniguruma.mjs";

globalThis.MonacoEnvironment = {
  getWorker: (_workerId, label) =>
    label === "typescript" || label === "javascript" ? new TypeScriptWorker() : new EditorWorker(),
};

// The typescript language service only hooks `onLanguage`, so the id and its brackets,
// comments and auto-closing pairs come from the basic language instead.
monaco.languages.register({ id: "typescript", extensions: [".ts", ".tsx"] });
monaco.languages.setLanguageConfiguration("typescript", typescriptConfiguration);

const highlighter = await createHighlighterCore({
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

shikiToMonaco(highlighter, monaco);

loader.config({ monaco });

/** The language service is megabytes of worker, so it is fetched once the editor has painted. */
export function startTypeScriptService(codeEditor: monaco.editor.IStandaloneCodeEditor) {
  // Safari has no `requestIdleCallback`; a timeout inside a frame lands after that frame paints.
  requestAnimationFrame(() =>
    setTimeout(() => {
      import("./typescript-service").then(
        (module) => module.startTypeScriptService(codeEditor),
        (error: unknown) => console.error("Failed to start the TypeScript service", error),
      );
    }),
  );
}
