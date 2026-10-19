// monaco-editor ships no declarations for these ESM entrypoints; the types below point at the
// ones the package publishes next to them.
declare module "monaco-editor/languages/definitions/typescript/typescript.js" {
  import type { languages } from "monaco-editor/editor/editor.api.js";

  export const conf: languages.LanguageConfiguration;
}

declare module "monaco-editor/languages/features/typescript/tsMode.js" {
  import type { LanguageServiceDefaults } from "monaco-editor/languages/features/typescript/register.js";

  export function setupTypeScript(defaults: LanguageServiceDefaults): void;
}
