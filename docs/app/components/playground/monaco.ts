import { loader } from "@monaco-editor/react";
import "monaco-editor/esm/vs/base/browser/ui/codicons/codicon/codicon.css";
import "monaco-editor/esm/vs/base/browser/ui/codicons/codicon/codicon-modifiers.css";
import { conf as typescriptConfiguration } from "monaco-editor/esm/vs/basic-languages/typescript/typescript.js";
import "monaco-editor/esm/vs/editor/browser/coreCommands.js";
import "monaco-editor/esm/vs/editor/browser/widget/codeEditor/codeEditorWidget.js";
import "monaco-editor/esm/vs/editor/contrib/bracketMatching/browser/bracketMatching.js";
import "monaco-editor/esm/vs/editor/contrib/clipboard/browser/clipboard.js";
import "monaco-editor/esm/vs/editor/contrib/comment/browser/comment.js";
import "monaco-editor/esm/vs/editor/contrib/contextmenu/browser/contextmenu.js";
import "monaco-editor/esm/vs/editor/contrib/find/browser/findController.js";
import "monaco-editor/esm/vs/editor/contrib/folding/browser/folding.js";
import "monaco-editor/esm/vs/editor/contrib/format/browser/formatActions.js";
import "monaco-editor/esm/vs/editor/contrib/hover/browser/hoverContribution.js";
import "monaco-editor/esm/vs/editor/contrib/indentation/browser/indentation.js";
import "monaco-editor/esm/vs/editor/contrib/lineSelection/browser/lineSelection.js";
import "monaco-editor/esm/vs/editor/contrib/linesOperations/browser/linesOperations.js";
import "monaco-editor/esm/vs/editor/contrib/parameterHints/browser/parameterHints.js";
import "monaco-editor/esm/vs/editor/contrib/suggest/browser/suggestController.js";
import "monaco-editor/esm/vs/editor/contrib/wordHighlighter/browser/wordHighlighter.js";
import "monaco-editor/esm/vs/editor/contrib/wordOperations/browser/wordOperations.js";
import * as monaco from "monaco-editor/esm/vs/editor/editor.api.js";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker.js?worker";
import "monaco-editor/esm/vs/language/typescript/monaco.contribution.js";
import TypeScriptWorker from "monaco-editor/esm/vs/language/typescript/ts.worker.js?worker";
import { languageId, registerSyntaxHighlighting } from "./syntax-highlighting";

export { startTypeScriptService } from "./typescript-service";

globalThis.MonacoEnvironment = {
  getWorker: (_workerId, label) =>
    label === "typescript" || label === "javascript" ? new TypeScriptWorker() : new EditorWorker(),
};

// The typescript language service only hooks `onLanguage`, so the id and its brackets,
// comments and auto-closing pairs come from the basic language instead.
monaco.languages.register({ id: languageId, extensions: [".ts", ".tsx"] });
monaco.languages.setLanguageConfiguration(languageId, typescriptConfiguration);

registerSyntaxHighlighting(monaco);

loader.config({ monaco });
