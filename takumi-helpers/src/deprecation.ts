let warned = false;

/** Hides the deprecated `stylesheets` alias from spreads, warning once when read. */
export function hideStylesheetsAlias(result: object): void {
  Object.defineProperty(result, "stylesheets", { enumerable: false });
}

export function warnStylesheetsDeprecated(): void {
  if (warned) return;
  warned = true;
  console.warn("takumi: the `stylesheets` result field is deprecated, use `css` instead.");
}
