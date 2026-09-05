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
  let collectedAttributes: Record<string, string> | undefined;

  for (const attributeName in props) {
    if (!Object.hasOwn(props, attributeName)) {
      continue;
    }

    const attributeValue = props[attributeName];

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

    collectedAttributes ??= {};
    collectedAttributes[attributeName] = attributeValue === true ? "" : String(attributeValue);
  }

  return collectedAttributes;
}
