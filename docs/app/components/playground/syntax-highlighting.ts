import type { Monaco } from "@monaco-editor/react";
import githubDark from "@shikijs/themes/github-dark-default";
import githubLight from "@shikijs/themes/github-light-default";
import type { ParsedLine, TokenType } from "sugar-high/core";
import { parse, SugarHigh } from "sugar-high/core";
import * as typescript from "sugar-high/lang/typescript";

type TextModel = ReturnType<Monaco["editor"]["getModels"]>[number];
type ThemeData = Parameters<Monaco["editor"]["defineTheme"]>[1];
type ShikiTheme = typeof githubDark;

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
 * The TextMate scope GitHub's themes colour each sugar-high token with. `sign` covers operators and
 * punctuation alike, so it stays at the editor foreground rather than painting braces keyword red.
 */
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

/** Resolves a scope the way TextMate does: the most specific prefix an entry lists wins. */
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
  monaco.editor.defineTheme(darkTheme, themeDataOf(githubDark, "vs-dark"));
  monaco.editor.defineTheme(lightTheme, themeDataOf(githubLight, "vs"));

  monaco.languages.setTokensProvider(languageId, createTokensProvider(monaco));
}
