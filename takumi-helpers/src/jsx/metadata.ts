import type { CSSProperties } from "react";
import { defaultStylePresets } from "./style-presets";

export type HtmlProps = {
  className?: string;
  class?: string;
  id?: string;
  style?: string | CSSProperties;
  dir?: string;
  [key: string]: unknown;
};

export function getPresets(
  defaultStyles?: typeof defaultStylePresets | false,
): typeof defaultStylePresets | undefined {
  if (defaultStyles === false) {
    return;
  }

  return defaultStyles ?? defaultStylePresets;
}

export function extractAttributes(
  props: HtmlProps,
  tailwindClassesProperty: string,
): Record<string, string> | undefined {
  const collectedAttributes: Record<string, string> = {};

  for (const [attributeName, attributeValue] of Object.entries(props)) {
    if (
      attributeName === "children" ||
      attributeName === "className" ||
      attributeName === "class" ||
      attributeName === "id" ||
      attributeName === "style" ||
      attributeName === tailwindClassesProperty ||
      attributeName === "ref" ||
      attributeName === "key" ||
      attributeName === "dangerouslySetInnerHTML" ||
      attributeName === "suppressHydrationWarning"
    ) {
      continue;
    }

    if (attributeValue === undefined || attributeValue === null || attributeValue === false) {
      continue;
    }

    if (typeof attributeValue === "function" || typeof attributeValue === "symbol") {
      continue;
    }

    if (typeof attributeValue === "object") {
      continue;
    }

    collectedAttributes[attributeName] = attributeValue === true ? "" : String(attributeValue);
  }

  if (Object.keys(collectedAttributes).length === 0) {
    return;
  }

  return collectedAttributes;
}
