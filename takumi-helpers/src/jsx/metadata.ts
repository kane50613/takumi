import type { Declarations } from "../types";
import { defaultStylePresets, type StylePresets } from "./style-presets";

export type HtmlProps = {
  className?: string;
  class?: string;
  id?: string;
  style?: string | Declarations;
  dir?: string;
  lang?: string;
  [key: string]: unknown;
};

export function getPresets(defaultStyles?: StylePresets | false): StylePresets | undefined {
  if (defaultStyles === false) {
    return;
  }

  return defaultStyles ?? defaultStylePresets;
}

function isPresetKey(presets: StylePresets, key: string): key is keyof StylePresets {
  return key in presets;
}

/** The preset for a tag, where an `input[type=…]` entry outranks the bare tag. */
export function presetFor(
  presets: StylePresets | undefined,
  tagName: string,
  type: unknown,
): Declarations | undefined {
  if (!presets) {
    return;
  }

  const typed = typeof type === "string" ? `${tagName}[type=${type.trim().toLowerCase()}]` : "";

  if (isPresetKey(presets, typed)) {
    return presets[typed];
  }

  return isPresetKey(presets, tagName) ? presets[tagName] : undefined;
}

// React spells these three HTML attributes its own way.
const reactAttributeNames: Record<string, string> = {
  htmlFor: "for",
  defaultValue: "value",
  defaultChecked: "checked",
};

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
    collectedAttributes[reactAttributeNames[attributeName] ?? attributeName] =
      attributeValue === true ? "" : String(attributeValue);
  }

  return collectedAttributes;
}
