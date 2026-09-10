// monaco-editor ships no declarations for these deep ESM entrypoints; the types below point at
// the ones the package publishes for its root entry.
declare module "monaco-editor/esm/vs/basic-languages/typescript/typescript.js" {
  import type { languages } from "monaco-editor/esm/vs/editor/editor.api.js";

  export const conf: languages.LanguageConfiguration;
}

declare module "monaco-editor/esm/vs/language/typescript/monaco.contribution.js" {
  import type { typescript } from "monaco-editor";

  export const typescriptDefaults: typescript.LanguageServiceDefaults;
  export const JsxEmit: typeof typescript.JsxEmit;
  export const ModuleKind: typeof typescript.ModuleKind;
  export const ModuleResolutionKind: typeof typescript.ModuleResolutionKind;
  export const ScriptTarget: typeof typescript.ScriptTarget;
}
