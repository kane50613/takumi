import { syntaxTree } from "@codemirror/language";
import { RangeSetBuilder, type Text } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  type EditorView,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";
import type { SyntaxNode, Tree } from "@lezer/common";

/** TextMate scopes the VS Code TypeScript grammar assigns from context that Lezer tags lack. */
export const SCOPE_CLASSES = {
  "cm-scope-constant": "variable.other.constant",
  "cm-scope-function": "entity.name.function",
  "cm-scope-primitive": "support.type.primitive",
  "cm-scope-export-default": "meta.export.default",
  "cm-scope-operator": "keyword.operator",
  "cm-scope-embedded": "punctuation.section.embedded",
} as const;

type ScopeClass = keyof typeof SCOPE_CLASSES;

const PRIMITIVE_TYPES = new Set([
  "any",
  "bigint",
  "boolean",
  "never",
  "null",
  "number",
  "object",
  "string",
  "symbol",
  "undefined",
  "unknown",
  "void",
]);

const CONSTANT_NAME = /^[A-Z][A-Z0-9_]*$/;

const marks = Object.fromEntries(
  Object.keys(SCOPE_CLASSES).map((name) => [name, Decoration.mark({ class: name })]),
) as Record<ScopeClass, Decoration>;

function definitionClass(node: SyntaxNode): ScopeClass | undefined {
  const declaration = node.parent;

  // Lezer's own `FunctionDeclaration/VariableDefinition` rule misses buffer nodes a cursor
  // enters sideways: https://github.com/lezer-parser/common/blob/main/src/tree.ts (matchContext)
  if (declaration?.name === "FunctionDeclaration") return "cm-scope-function";

  if (declaration?.name !== "VariableDeclaration" || declaration.firstChild?.name !== "const") {
    return undefined;
  }

  const value = node.nextSibling?.name === "Equals" ? node.nextSibling.nextSibling : undefined;

  return value?.name === "ArrowFunction" || value?.name === "FunctionExpression"
    ? "cm-scope-function"
    : "cm-scope-constant";
}

function isDefaultExport(declaration: SyntaxNode | null) {
  const exported = declaration?.parent;

  return exported?.name === "ExportDeclaration" && declaration?.prevSibling?.name === "default";
}

export function scopeMarks(tree: Tree, doc: Text, from: number, to: number) {
  const ranges: { from: number; to: number; mark: Decoration }[] = [];
  const add = (from: number, to: number, name: ScopeClass) => {
    ranges.push({ from, to, mark: marks[name] });
  };

  tree.iterate({
    from,
    to,
    enter(node) {
      switch (node.name) {
        case "VariableDefinition": {
          const name = definitionClass(node.node);

          if (name) add(node.from, node.to, name);
          break;
        }
        case "VariableName": {
          const text = doc.sliceString(node.from, node.to);

          if (text === "undefined" || CONSTANT_NAME.test(text)) {
            add(node.from, node.to, "cm-scope-constant");
          }
          break;
        }
        case "TypeName":
          if (PRIMITIVE_TYPES.has(doc.sliceString(node.from, node.to))) {
            add(node.from, node.to, "cm-scope-primitive");
          }
          break;
        case "ParamList":
          if (isDefaultExport(node.node.parent)) {
            add(node.from, node.from + 1, "cm-scope-export-default");
            add(node.to - 1, node.to, "cm-scope-export-default");
          }
          break;
        case "TypeAnnotation":
        case "Spread":
          add(node.from, node.from + (node.name === "Spread" ? 3 : 1), "cm-scope-operator");
          break;
        case "JSXEscape":
          add(node.from, node.from + 1, "cm-scope-embedded");
          add(node.to - 1, node.to, "cm-scope-embedded");
          break;
      }
    },
  });

  return ranges;
}

function scopeDecorations(view: EditorView) {
  const builder = new RangeSetBuilder<Decoration>();
  const ranges = view.visibleRanges
    .flatMap(({ from, to }) => scopeMarks(syntaxTree(view.state), view.state.doc, from, to))
    .sort((a, b) => a.from - b.from || a.to - b.to);

  for (const { from, to, mark } of ranges) builder.add(from, to, mark);

  return builder.finish();
}

/** Colours the tokens VS Code colours by context: const names, primitive types, JSX braces. */
export const textmateScopes = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = scopeDecorations(view);
    }

    update(update: ViewUpdate) {
      if (
        update.docChanged ||
        update.viewportChanged ||
        syntaxTree(update.state) !== syntaxTree(update.startState)
      ) {
        this.decorations = scopeDecorations(update.view);
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);
