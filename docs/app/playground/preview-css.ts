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
  if (typeof entry === "string") return entry;

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

/**
 * Serializes the animations the deprecated `keyframes` option carries.
 *
 * @deprecated Will be removed in v3, with the option itself.
 */
export function keyframesToCss(keyframes: NonNullable<PlaygroundOptions["keyframes"]>): string {
  const rules = Array.isArray(keyframes)
    ? keyframes.map((rule) => {
        const body = rule.keyframes
          .map(
            (frame) =>
              `${frame.offsets.map((offset) => `${offset * 100}%`).join(", ")} { ${declarationsToCss(frame.declarations)} }`,
          )
          .join(" ");
        return `@keyframes ${rule.name} { ${body} }`;
      })
    : Object.entries(keyframes).map(([name, offsets]) => {
        const body = Object.entries(offsets)
          .map(([offset, declarations]) => `${offset} { ${declarationsToCss(declarations)} }`)
          .join(" ");
        return `@keyframes ${name} { ${body} }`;
      });

  return rules.join("\n");
}
