import type { Declarations } from "../types";
import { defaultStylePresets } from "./style-presets";

export type HtmlProps = {
  className?: string;
  class?: string;
  id?: string;
  style?: string | Declarations;
  dir?: string;
  lang?: string;
  [key: string]: unknown;
};

const nonAttributeProps = new Set([
  "children",
  "className",
  "class",
  "id",
  "style",
  "ref",
  "key",
  "dangerouslySetInnerHTML",
  "suppressHydrationWarning",
]);

const attributeValueTypes = new Set(["string", "number", "bigint", "boolean"]);

export function getPresets(
  defaultStyles?: typeof defaultStylePresets | false,
): typeof defaultStylePresets | undefined {
  if (defaultStyles === false) {
    return;
  }

  return defaultStyles ?? defaultStylePresets;
}

export function presetFor(
  presets: typeof defaultStylePresets | undefined,
  tagName: string | undefined,
): Declarations | undefined {
  return presets && tagName !== undefined && tagName in presets
    ? presets[tagName as keyof typeof presets]
    : undefined;
}

export function extractAttributes(
  props: HtmlProps,
  tailwindClassesProperty: string,
): Record<string, string> | undefined {
  let attributes: Record<string, string> | undefined;

  for (const [name, value] of Object.entries(props)) {
    if (nonAttributeProps.has(name) || name === tailwindClassesProperty) continue;

    if (value === false || !attributeValueTypes.has(typeof value)) continue;

    attributes ??= {};
    attributes[name] = value === true ? "" : String(value);
  }

  return attributes;
}
