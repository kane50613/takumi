function declarationsToCss(declarations: object): string {
  return Object.entries(declarations)
    .map(
      ([property, value]) =>
        `${property.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}: ${value};`,
    )
    .join(" ");
}

/**
 * Serializes structured keyframes into `@keyframes` rules. The engine takes
 * them as an object; the browser preview pane only understands CSS.
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
