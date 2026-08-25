/** A nested tree of variable values; keys join with `-` into one variable name. */
export type CssVariableTree = { [key: string]: string | number | CssVariableTree };

/**
 * Flattens a nested tree into the flat map the `cssVariables` option takes:
 * `{ color: { brand: { 500: "#5b21b6" } } }` becomes `{ "--color-brand-500": "#5b21b6" }`.
 */
export function cssVariables(tree: CssVariableTree): Record<string, string> {
  const flat: Record<string, string> = {};

  flatten(tree, "-", flat);
  return flat;
}

function flatten(tree: CssVariableTree, prefix: string, into: Record<string, string>) {
  for (const [key, value] of Object.entries(tree)) {
    // Tailwind v3 configs spell a scale's bare value as `DEFAULT`.
    const name = key === "DEFAULT" ? prefix : `${prefix}-${key}`;

    if (typeof value === "object") {
      flatten(value, name, into);
    } else {
      into[name] = String(value);
    }
  }
}
