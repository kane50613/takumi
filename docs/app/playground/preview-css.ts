import type { AnimationRule, CssInput } from "takumi-js";

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

/**
 * Serializes the animation entries of a `css` list. The engine reads them as
 * objects; the browser preview pane only understands CSS.
 */
export function animationsToCss(css: readonly CssInput[]): string {
  const rules: string[] = [];

  for (const entry of css) {
    if (!isAnimationRule(entry)) continue;

    const body = entry.steps
      .map(
        (step: AnimationRule["steps"][number]) =>
          `${step.offset} { ${declarationsToCss(step.style ?? {})} }`,
      )
      .join(" ");

    rules.push(`@keyframes ${entry.keyframes} { ${body} }`);
  }

  return rules.join("\n");
}
