import type { Monaco } from "@monaco-editor/react";
import type { ParsedLine, TokenType } from "sugar-high/core";
import { parse, SugarHigh } from "sugar-high/core";
import * as typescript from "sugar-high/lang/typescript";

type TextModel = ReturnType<Monaco["editor"]["getModels"]>[number];
type ThemeData = Parameters<Monaco["editor"]["defineTheme"]>[1];

/**
 * Monaco hands the provider one line at a time, so the state carries the line number. It also
 * carries the token left open at the end of the line: Monaco stops re-tokenizing as soon as an end
 * state equals the one it stored, and every line below a newly opened comment or template literal
 * needs new colours even though its number did not change.
 */
type LineState = {
  lineIndex: number;
  openToken: number | undefined;
  clone(): LineState;
  equals(other: LineState): boolean;
};

type ParsedModel = {
  version: number;
  lines: readonly ParsedLine[];
  /** Per line, the type of the token that runs past its end, aligned with `lines`. */
  openTokens: readonly (number | undefined)[];
};

export const darkTheme = "takumi-dark";
export const lightTheme = "takumi-light";

const languageId = "typescript";

const breakToken = SugarHigh.TokenMap.get("break");

/**
 * Colours follow GitHub's default themes. sugar-high has no separate function token, so calls stay
 * in the identifier colour instead of GitHub's purple.
 */
const themes: Record<string, ThemeData & { rules: { token: string; foreground: string }[] }> = {
  [darkTheme]: {
    base: "vs-dark",
    inherit: true,
    rules: tokenRules({
      foreground: "e6edf3",
      keyword: "ff7b72",
      string: "a5d6ff",
      comment: "8b949e",
      class: "79c0ff",
      property: "79c0ff",
      entity: "7ee787",
    }),
    colors: {
      "editor.background": "#0d1117",
      "editor.foreground": "#e6edf3",
      "editor.lineHighlightBackground": "#6e76811a",
      "editor.selectionHighlightBackground": "#3fb95040",
      "editorCursor.foreground": "#2f81f7",
      "editorIndentGuide.activeBackground": "#e6edf33d",
      "editorIndentGuide.background": "#e6edf31f",
      "editorLineNumber.activeForeground": "#e6edf3",
      "editorLineNumber.foreground": "#6e7681",
      "editorWhitespace.foreground": "#484f58",
      "editorWidget.background": "#161b22",
      "editorBracketMatch.background": "#3fb95040",
      "editorBracketMatch.border": "#3fb95099",
    },
  },
  [lightTheme]: {
    base: "vs",
    inherit: true,
    rules: tokenRules({
      foreground: "1f2328",
      keyword: "cf222e",
      string: "0a3069",
      comment: "6e7781",
      class: "0550ae",
      property: "0550ae",
      entity: "116329",
    }),
    colors: {
      "editor.background": "#ffffff",
      "editor.foreground": "#1f2328",
      "editor.lineHighlightBackground": "#eaeef280",
      "editor.selectionHighlightBackground": "#4ac26b40",
      "editorCursor.foreground": "#0969da",
      "editorIndentGuide.activeBackground": "#1f23283d",
      "editorIndentGuide.background": "#1f23281f",
      "editorLineNumber.activeForeground": "#1f2328",
      "editorLineNumber.foreground": "#8c959f",
      "editorWhitespace.foreground": "#afb8c1",
      "editorWidget.background": "#ffffff",
      "editorBracketMatch.background": "#4ac26b40",
      "editorBracketMatch.border": "#4ac26b99",
    },
  },
};

function tokenRules(
  palette: { foreground: string } & Partial<Record<TokenType, string>>,
): { token: string; foreground: string }[] {
  const { foreground, ...tokens } = palette;

  return [
    { token: "", foreground },
    ...Object.entries(tokens).map(([type, color]) => ({ token: scopeOf(type), foreground: color })),
  ];
}

function scopeOf(type: string) {
  return `sh.${type}`;
}

/** Token types the themes leave at the editor foreground, so they need no scope of their own. */
const plainTypes = new Set<TokenType>(["identifier", "sign", "space", "break", "jsxliterals"]);

/**
 * sugar-high splits a token that spans lines into one token per line without a break between them,
 * so the tokens it emits for a break are the only line ends with nothing left open.
 */
function openTokensOf(tokens: readonly [number, string][]) {
  const openTokens: (number | undefined)[] = [];

  for (const [type, value] of tokens) {
    if (type === breakToken) {
      openTokens.push(undefined);
      continue;
    }

    for (let count = value.split("\n").length - 1; count > 0; count--) {
      openTokens.push(type);
    }
  }

  return openTokens;
}

function parseCode(code: string): Omit<ParsedModel, "version"> {
  let rawTokens: [number, string][] = [];

  const { lines } = parse(code, {
    ...typescript,
    tokenize: (source, options) => {
      rawTokens = typescript.tokenize(source, options);

      return rawTokens;
    },
  });

  return { lines, openTokens: openTokensOf(rawTokens) };
}

function tokensOf(parsedLine: ParsedLine) {
  const tokens: { startIndex: number; scopes: string }[] = [];
  let startIndex = 0;

  for (const { type, value } of parsedLine.tokens) {
    if (!value) {
      continue;
    }

    const scopes = plainTypes.has(type) ? "" : scopeOf(type);

    if (tokens.at(-1)?.scopes !== scopes) {
      tokens.push({ startIndex, scopes });
    }

    startIndex += value.length;
  }

  return tokens;
}

/**
 * sugar-high parses a whole document rather than a line at a time, so the provider looks the line up
 * in the cached parse of the model it belongs to. Monaco names no model in `tokenize`, so the
 * provider keeps the last model that matched and only rescans when it stops matching. A line that
 * matches no model is parsed on its own, which loses the multi-line comment and template context.
 */
function createTokensProvider(monaco: Monaco) {
  const parsedModels = new WeakMap<TextModel, ParsedModel>();
  let boundModel: TextModel | undefined;

  const parseOf = (model: TextModel) => {
    const version = model.getVersionId();
    const cached = parsedModels.get(model);

    if (cached && cached.version === version) {
      return cached;
    }

    const parsed = {
      version,
      ...parseCode(model.getValue(monaco.editor.EndOfLinePreference.LF)),
    };

    parsedModels.set(model, parsed);

    return parsed;
  };

  const lineAt = (lineIndex: number, line: string) => {
    const models =
      boundModel && !boundModel.isDisposed()
        ? [boundModel, ...monaco.editor.getModels()]
        : monaco.editor.getModels();

    for (const model of models) {
      if (model.getLanguageId() !== languageId) {
        continue;
      }

      const parsed = parseOf(model);

      if (parsed.lines[lineIndex]?.value === line) {
        boundModel = model;

        return { parsedLine: parsed.lines[lineIndex], openToken: parsed.openTokens[lineIndex] };
      }
    }
  };

  const createState = (lineIndex: number, openToken: number | undefined): LineState => ({
    lineIndex,
    openToken,
    clone: () => createState(lineIndex, openToken),
    equals: (other) => other.lineIndex === lineIndex && other.openToken === openToken,
  });

  return {
    getInitialState: () => createState(0, undefined),
    tokenize(line: string, state: LineState) {
      const found = lineAt(state.lineIndex, line);

      if (found) {
        return {
          tokens: tokensOf(found.parsedLine),
          endState: createState(state.lineIndex + 1, found.openToken),
        };
      }

      const [parsedLine] = parse(line, typescript).lines;

      return {
        tokens: parsedLine ? tokensOf(parsedLine) : [],
        endState: createState(state.lineIndex + 1, state.openToken),
      };
    },
  };
}

/** Registers sugar-high tokenization and the two editor themes on a Monaco instance. */
export function registerSyntaxHighlighting(monaco: Monaco) {
  for (const [name, data] of Object.entries(themes)) {
    monaco.editor.defineTheme(name, data);
  }

  monaco.languages.setTokensProvider(languageId, createTokensProvider(monaco));
}
