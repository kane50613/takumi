import type { AnimationRule, CssInput, StyleRule } from "takumi-js";

function declarationsToCss(declarations: object): string {
  return Object.entries(declarations)
    .map(
      ([property, value]) =>
        `${property.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}: ${value};`,
    )
    .join(" ");
}

function isAnimationRule(entry: CssInput): entry is AnimationRule {
  return typeof entry === "object" && "keyframes" in entry;
}

function styleRuleToCss(rule: StyleRule): string {
  const nested = (rule.rules ?? []).map((child: StyleRule) => styleRuleToCss(child)).join(" ");

  return `${rule.selector} { ${declarationsToCss(rule.style ?? {})} ${nested} }`;
}

function animationRuleToCss(rule: AnimationRule): string {
  const body = rule.steps
    .map(
      (step: AnimationRule["steps"][number]) =>
        `${step.offset} { ${declarationsToCss(step.style ?? {})} }`,
    )
    .join(" ");

  return `@keyframes ${rule.keyframes} { ${body} }`;
}

/**
 * The CSS a `css` entry stands for. The engine reads a rule as an object; the
 * browser preview pane only understands text.
 */
export function cssEntryToText(entry: CssInput): string {
  if (typeof entry === "string") return entry;

  return isAnimationRule(entry) ? animationRuleToCss(entry) : styleRuleToCss(entry);
}

/**
 * Serializes the animations the deprecated `keyframes` option carries.
 *
 * @deprecated Removed in v3, with the option itself.
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
