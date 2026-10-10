import { text } from "../helpers";
import type { Node } from "../types";
import type { StylePresets } from "./style-presets";

const buttonLabels: Record<string, string> = { submit: "Submit", reset: "Reset", button: "" };

/** The text a push button `<input>` shows: its `value`, or the label HTML gives a submit or reset button without one. */
export function buttonLabel(attributes: Record<string, string> | undefined): string | undefined {
  const fallback = buttonLabels[attributes?.type?.trim().toLowerCase() ?? ""];

  if (fallback === undefined) {
    return;
  }

  return attributes?.value ?? fallback;
}

/** Whether a `<select>` lays its options out as a list box: `multiple`, or a `size` above one. */
export function isListBox(attributes: Record<string, string> | undefined): boolean {
  if (!attributes) {
    return false;
  }

  const digits = /^[\t\n\f\r ]*\+?([0-9]+)/.exec(attributes.size ?? "");
  const size = Number(digits?.[1]);

  return "multiple" in attributes || (size > 1 && size <= 4294967295);
}

interface CollectedOption {
  option: Node;
  /** Whether the option or the `<optgroup>` holding it carries `disabled`. */
  disabled: boolean;
}

/** Every `<option>` under `nodes`, in document order. */
function collectOptions(nodes: Node[], out: CollectedOption[], inDisabledGroup = false): void {
  for (const node of nodes) {
    if (node.tagName === "option") {
      out.push({
        option: node,
        disabled: inDisabledGroup || node.attributes?.disabled !== undefined,
      });
      continue;
    }

    if (node.type === "container" && node.children) {
      const disabledGroup = node.tagName === "optgroup" && node.attributes?.disabled !== undefined;

      collectOptions(node.children, out, disabledGroup);
    }
  }
}

function optionValue(option: Node): string {
  return option.attributes?.value ?? optionText(option);
}

function optionLabel(option: Node): string {
  const label = option.attributes?.label;

  if (label) {
    return label;
  }

  return optionText(option);
}

function optionText(option: Node): string {
  return nodeText(option)
    .split(/[\t\n\f\r ]+/)
    .filter(Boolean)
    .join(" ");
}

function nodeText(node: Node): string {
  if (node.type === "text") {
    return node.text;
  }

  if (node.type === "container") {
    return (node.children ?? []).map(nodeText).join("");
  }

  return "";
}

/**
 * Marks the options a React `value` or `defaultValue` on the `<select>` picks
 * as `selected`, the way React DOM renders them.
 */
export function selectValue(children: Node[], value: unknown): void {
  if (typeof value !== "string" && typeof value !== "number" && !Array.isArray(value)) {
    return;
  }

  const picked = new Set((Array.isArray(value) ? value : [value]).map(String));
  const options: CollectedOption[] = [];

  collectOptions(children, options);

  for (const { option } of options) {
    const attributes = { ...option.attributes };

    if (picked.has(optionValue(option))) {
      attributes.selected = "";
    } else {
      delete attributes.selected;
    }

    option.attributes = attributes;
  }
}

/**
 * Shows a closed `<select>` the way a drop-down does: the option it starts on
 * as the select's own text, with the option list itself out of the flow.
 *
 * Follows HTML's selectedness setting algorithm: the last `selected` option,
 * else the first enabled one.
 * https://html.spec.whatwg.org/multipage/form-elements.html#selectedness-setting-algorithm
 */
export function closeSelect(children: Node[], presets: StylePresets | undefined): Node[] {
  const options: CollectedOption[] = [];

  collectOptions(children, options);

  const shown = (
    options.findLast(({ option }) => option.attributes?.selected !== undefined) ??
    options.find(({ disabled }) => !disabled)
  )?.option;
  const hidden = children.map((child) =>
    child.tagName === "option" || child.tagName === "optgroup"
      ? { ...child, preset: { display: "none" as const } }
      : child,
  );

  if (!shown) {
    return hidden;
  }

  return [text({ text: optionLabel(shown), preset: presets?.span }), ...hidden];
}
