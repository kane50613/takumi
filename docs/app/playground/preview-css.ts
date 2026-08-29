import type { CssEntry } from "./schema";

function declarationsToCss(declarations: object): string {
  return Object.entries(declarations)
    .map(
      ([property, value]) =>
        `${property.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}: ${value};`,
    )
    .join(" ");
}

function styleRuleToCss(rule: Extract<CssEntry, { selector: string }>): string {
  return `${rule.selector} { ${declarationsToCss(rule.style ?? {})} ${groupToCss(rule.rules)} }`;
}

function groupToCss(rules: CssEntry[] | undefined): string {
  return (rules ?? []).map(cssEntryToText).join(" ");
}

/**
 * The CSS a `css` entry stands for. The engine reads a rule as an object; the
 * browser preview pane only understands text.
 */
export function cssEntryToText(entry: CssEntry): string {
  // The engine reads `@theme` as a `:root` rule; a browser drops the block whole.
  if (typeof entry === "string") return entry.replace(/@theme[^{]*\{/g, ":root {");

  if ("keyframes" in entry) {
    const body = entry.steps
      .map((step) => `${step.offset} { ${declarationsToCss(step.style ?? {})} }`)
      .join(" ");

    return `@keyframes ${entry.keyframes} { ${body} }`;
  }

  if ("media" in entry) return `@media ${entry.media} { ${groupToCss(entry.rules)} }`;
  if ("supports" in entry) return `@supports ${entry.supports} { ${groupToCss(entry.rules)} }`;

  if ("layer" in entry) {
    return entry.rules
      ? `@layer ${entry.layer} { ${groupToCss(entry.rules)} }`
      : `@layer ${entry.layer};`;
  }

  return styleRuleToCss(entry);
}
