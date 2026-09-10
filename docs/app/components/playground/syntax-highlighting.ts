import type { Monaco } from "@monaco-editor/react";
import githubDark from "@shikijs/themes/github-dark-default";
import githubLight from "@shikijs/themes/github-light-default";
import type { ParsedLine, TokenType } from "sugar-high/core";
import { parse, SugarHigh } from "sugar-high/core";
import * as typescript from "sugar-high/lang/typescript";

type TextModel = ReturnType<Monaco["editor"]["getModels"]>[number];
type ThemeData = Parameters<Monaco["editor"]["defineTheme"]>[1];
type ShikiTheme = typeof githubDark;

/** Monaco stops re-tokenizing once an end state repeats, so the state carries the token left open. */
type LineState = {
  lineIndex: number;
  openToken: number | undefined;
  clone(): LineState;
  equals(other: LineState): boolean;
};

export const darkTheme = "takumi-dark";
export const lightTheme = "takumi-light";

const languageId = "typescript";

const breakToken = SugarHigh.TokenMap.get("break");

/** `sign` covers punctuation as well as operators, so it stays at the editor foreground. */
const tokenScopes = {
  keyword: "keyword",
  string: "string",
  comment: "comment",
  class: "entity.name.type",
  entity: "entity.name.tag",
  property: "support.variable.property",
  jsxliterals: "meta.jsx.children",
  constant: "constant.numeric",
} satisfies Partial<Record<TokenType | "constant", string>>;

function scopeOf(type: string) {
  return `sh.${type}`;
}

/** TextMate resolves a scope to the most specific prefix an entry lists. */
function settingsOfScope(theme: ShikiTheme, scope: string) {
  const segments = scope.split(".");

  for (let length = segments.length; length > 0; length--) {
    const prefix = segments.slice(0, length).join(".");
    const entry = theme.tokenColors?.find(({ scope: entryScope }) =>
      Array.isArray(entryScope) ? entryScope.includes(prefix) : entryScope === prefix,
    );

    if (entry?.settings.foreground) {
      return entry.settings;
    }
  }
}

/** github-dark-default lists a colour ramp under `symbolIcon.constantForeground`, which Monaco rejects. */
const singleColor = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

function themeDataOf(theme: ShikiTheme, base: ThemeData["base"]): ThemeData {
  const colors = Object.fromEntries(
    Object.entries(theme.colors ?? {}).filter(([, color]) => singleColor.test(color)),
  );
  const rules = Object.entries(tokenScopes).flatMap(([type, scope]) => {
    const settings = settingsOfScope(theme, scope);

    return settings?.foreground
      ? [{ token: scopeOf(type), foreground: settings.foreground, fontStyle: settings.fontStyle }]
      : [];
  });

  return {
    base,
    inherit: true,
    colors,
    rules: [{ token: "", foreground: colors["editor.foreground"] }, ...rules],
  };
}

/** sugar-high splits a multi-line token per line, so its break tokens are the only closed line ends. */
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

function tokensOf(parsedLine: ParsedLine) {
  const tokens: { startIndex: number; scopes: string }[] = [];
  let startIndex = 0;

  for (const { type, value } of parsedLine.tokens) {
    if (!value) {
      continue;
    }

    const scopes =
      type === "class" && /^\d/.test(value)
        ? scopeOf("constant")
        : type in tokenScopes
          ? scopeOf(type)
          : "";

    if (tokens.at(-1)?.scopes !== scopes) {
      tokens.push({ startIndex, scopes });
    }

    startIndex += value.length;
  }

  return tokens;
}

/** Monaco names no model in `tokenize`, so lines are matched by text, which the playground's single model keeps unambiguous. */
function createTokensProvider(monaco: Monaco) {
  const parsedModels = new WeakMap<
    TextModel,
    { version: number; lines: readonly ParsedLine[]; openTokens: readonly (number | undefined)[] }
  >();
  let boundModel: TextModel | undefined;

  const parseOf = (model: TextModel) => {
    const version = model.getVersionId();
    const cached = parsedModels.get(model);

    if (cached && cached.version === version) {
      return cached;
    }

    let rawTokens: [number, string][] = [];

    const { lines } = parse(model.getValue(monaco.editor.EndOfLinePreference.LF), {
      ...typescript,
      tokenize: (source, options) => {
        rawTokens = typescript.tokenize(source, options);

        return rawTokens;
      },
    });

    const parsed = { version, lines, openTokens: openTokensOf(rawTokens) };

    parsedModels.set(model, parsed);

    return parsed;
  };

  const lineAt = (lineIndex: number, line: string) => {
    const matches = monaco.editor
      .getModels()
      .filter(
        (model: TextModel) =>
          model.getLanguageId() === languageId && parseOf(model).lines[lineIndex]?.value === line,
      );
    const model: TextModel | undefined =
      matches.length > 1 ? matches.find((match: TextModel) => match === boundModel) : matches[0];

    if (!model) {
      return;
    }

    boundModel = model;

    const parsed = parseOf(model);

    return { parsedLine: parsed.lines[lineIndex], openToken: parsed.openTokens[lineIndex] };
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

export function registerSyntaxHighlighting(monaco: Monaco) {
  monaco.editor.defineTheme(darkTheme, themeDataOf(githubDark, "vs-dark"));
  monaco.editor.defineTheme(lightTheme, themeDataOf(githubLight, "vs"));

  monaco.languages.setTokensProvider(languageId, createTokensProvider(monaco));
}
