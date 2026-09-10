import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { type Tag, tags } from "@lezer/highlight";
import type { ThemeRegistration } from "shiki";

/** Lezer tags mapped onto the TextMate scopes the GitHub themes colour. */
const TAG_SCOPES: [Tag | Tag[], string][] = [
  [tags.keyword, "keyword"],
  [[tags.controlKeyword, tags.moduleKeyword, tags.definitionKeyword], "keyword.control"],
  [[tags.string, tags.special(tags.string)], "string"],
  [[tags.comment, tags.lineComment, tags.blockComment, tags.docComment], "comment"],
  [[tags.number, tags.bool, tags.null, tags.atom], "constant"],
  [[tags.function(tags.variableName), tags.function(tags.propertyName)], "entity.name.function"],
  [tags.definition(tags.variableName), "meta.definition.variable"],
  [[tags.typeName, tags.className, tags.namespace], "entity.name.type"],
  [tags.tagName, "entity.name.tag"],
  [tags.attributeName, "entity.other.attribute-name"],
  [tags.propertyName, "variable.other.property"],
  [tags.regexp, "string.regexp"],
  [[tags.self, tags.standard(tags.variableName)], "variable.language"],
];

function color(theme: ThemeRegistration, key: string) {
  // `symbolIcon.constantForeground` holds a ramp of hex strings, not a colour.
  const value: unknown = theme.colors?.[key];

  return typeof value === "string" ? value : undefined;
}

/** The colour of the most specific dotted scope prefix, the way TextMate resolves one. */
function scopeColor(theme: ThemeRegistration, scope: string) {
  let best: { depth: number; foreground: string } | undefined;

  for (const rule of theme.tokenColors ?? theme.settings ?? []) {
    const foreground = rule.settings?.foreground;

    if (!foreground) continue;

    const scopes = typeof rule.scope === "string" ? rule.scope.split(",") : (rule.scope ?? []);

    for (const candidate of scopes) {
      const prefix = candidate.trim();

      if (scope !== prefix && !scope.startsWith(`${prefix}.`)) continue;

      const depth = prefix.split(".").length;

      if (!best || depth >= best.depth) best = { depth, foreground };
    }
  }

  return best?.foreground;
}

export function editorTheme(theme: ThemeRegistration): Extension {
  const background = color(theme, "editor.background") ?? "#ffffff";
  const foreground = color(theme, "editor.foreground") ?? "#000000";
  const gutterForeground = color(theme, "editorLineNumber.foreground") ?? foreground;
  const selection =
    color(theme, "editor.selectionBackground") ??
    color(theme, "list.activeSelectionBackground") ??
    "#3392ff44";
  const matchingBracket = color(theme, "editorBracketMatch.background") ?? selection;
  const lineHighlight = color(theme, "editor.lineHighlightBackground") ?? "transparent";

  const highlightStyle = HighlightStyle.define(
    TAG_SCOPES.flatMap(([tag, scope]) => {
      const foreground = scopeColor(theme, scope);

      return foreground ? [{ tag, color: foreground }] : [];
    }).concat({ tag: tags.operator, color: foreground }),
    { themeType: theme.type },
  );

  return [
    EditorView.theme(
      {
        "&": { backgroundColor: background, color: foreground },
        ".cm-content": { caretColor: color(theme, "editorCursor.foreground") ?? foreground },
        ".cm-cursor, .cm-dropCursor": {
          borderLeftColor: color(theme, "editorCursor.foreground") ?? foreground,
        },
        ".cm-activeLine": { backgroundColor: lineHighlight },
        "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
          backgroundColor: selection,
        },
        ".cm-gutters": {
          backgroundColor: background,
          color: gutterForeground,
          borderRight: "none",
        },
        ".cm-activeLineGutter": {
          backgroundColor: lineHighlight,
          color: color(theme, "editorLineNumber.activeForeground") ?? foreground,
        },
        ".cm-foldPlaceholder": {
          backgroundColor: matchingBracket,
          color: foreground,
          border: "none",
        },
        "&.cm-focused .cm-matchingBracket, &.cm-focused .cm-nonmatchingBracket": {
          backgroundColor: matchingBracket,
          outline: `1px solid ${color(theme, "editorBracketMatch.border") ?? matchingBracket}`,
        },
        ".cm-tooltip": {
          backgroundColor: color(theme, "editorWidget.background") ?? background,
          border: `1px solid ${color(theme, "panel.border") ?? gutterForeground}`,
          borderRadius: "6px",
          color: foreground,
        },
        ".cm-tooltip-hover": {
          padding: "4px 8px",
          fontFamily: "var(--font-mono)",
          maxWidth: "42em",
        },
        ".cm-tooltip .cm-tooltip-arrow:before": {
          borderTopColor: color(theme, "panel.border") ?? gutterForeground,
          borderBottomColor: color(theme, "panel.border") ?? gutterForeground,
        },
        ".cm-tooltip .cm-tooltip-arrow:after": {
          borderTopColor: color(theme, "editorWidget.background") ?? background,
          borderBottomColor: color(theme, "editorWidget.background") ?? background,
        },
        ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
          backgroundColor: color(theme, "list.activeSelectionBackground") ?? selection,
          color: foreground,
        },
        ".cm-panels": {
          backgroundColor: color(theme, "editorWidget.background") ?? background,
          color: foreground,
        },
        ".cm-searchMatch": {
          backgroundColor: color(theme, "editor.selectionHighlightBackground") ?? selection,
        },
      },
      { dark: theme.type === "dark" },
    ),
    syntaxHighlighting(highlightStyle),
  ];
}
