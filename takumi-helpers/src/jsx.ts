import {
  type ComponentProps,
  type CSSProperties,
  isValidElement,
  type JSX,
  type ReactElement,
  type ReactNode,
} from "react";
import type { LengthUnit } from "./bindings/LengthUnit";
import { container, em, image, percentage, rem, text, vh, vw } from "./helpers";
import type { Node, SidesValue, StyleInput } from "./types";

type Nodes = Node | Nodes[];

function flattenNodes(nodes: Nodes[]): Node[] {
  const result: Node[] = [];
  for (const node of nodes) {
    if (Array.isArray(node)) {
      result.push(...flattenNodes(node));
    } else {
      result.push(node);
    }
  }
  return result;
}

export async function fromJsx(element: ReactNode): Promise<Nodes | undefined> {
  if (element === undefined || element === null) return;

  if (element instanceof Promise) return fromJsx(await element);

  if (typeof element === "object" && Symbol.iterator in element)
    return collectIterable(element);

  if (isValidElement(element)) {
    return processReactElement(element);
  }

  return text(String(element));
}

async function processReactElement(
  element: ReactElement,
): Promise<Nodes | undefined> {
  if (typeof element.type === "function") {
    const FunctionComponent = element.type as (props: unknown) => ReactNode;
    return fromJsx(FunctionComponent(element.props));
  }

  // Handle React fragments
  if (
    typeof element.type === "symbol" &&
    element.type === Symbol.for("react.fragment")
  ) {
    const children = await collectChildren(element);
    return children || [];
  }

  if (isHtmlElement(element, "img") && element.props.src) {
    return image(element.props.src, parseStyle(element.props.style));
  }

  const children = await collectChildren(element);
  const style = extractStyleFromProps(element.props);

  return container({
    children,
    ...parseStyle(style),
  });
}

function extractStyleFromProps(props: any): CSSProperties | undefined {
  return typeof props === "object" && props && "style" in props
    ? (props.style as CSSProperties)
    : undefined;
}

function isHtmlElement<T extends keyof JSX.IntrinsicElements>(
  element: ReactElement,
  type: T,
): element is ReactElement<ComponentProps<T>> {
  return element.type === type;
}

async function collectChildren(
  element: ReactElement,
): Promise<Node[] | undefined> {
  if (
    typeof element.props !== "object" ||
    element.props === null ||
    !("children" in element.props)
  )
    return;

  const result = await fromJsx(element.props.children as ReactNode);
  if (!result) return;

  return Array.isArray(result) ? flattenNodes(result) : [result];
}

async function collectIterable(iterable: Iterable<ReactNode>): Promise<Node[]> {
  const children = await Promise.all(Array.from(iterable).map(fromJsx));

  return flattenNodes(children.filter((x): x is Nodes => x !== undefined));
}

function parseStyle(source?: CSSProperties): StyleInput | undefined {
  if (!source) return;

  const style: Partial<StyleInput> = {};

  // Parse dimensions
  if (source.width !== undefined) style.width = parseLengthUnit(source.width);
  if (source.height !== undefined)
    style.height = parseLengthUnit(source.height);
  if (source.maxWidth !== undefined)
    style.max_width = parseLengthUnit(source.maxWidth);
  if (source.maxHeight !== undefined)
    style.max_height = parseLengthUnit(source.maxHeight);
  if (source.minWidth !== undefined)
    style.min_width = parseLengthUnit(source.minWidth);
  if (source.minHeight !== undefined)
    style.min_height = parseLengthUnit(source.minHeight);

  // Parse spacing
  if (source.padding !== undefined)
    style.padding = parseSidesValue(source.padding);
  if (source.margin !== undefined)
    style.margin = parseSidesValue(source.margin);

  // Parse flex properties
  if (source.flexDirection !== undefined) {
    style.flex_direction = source.flexDirection as any;
  }
  if (source.justifyContent !== undefined) {
    style.justify_content = source.justifyContent;
  }
  if (source.alignItems !== undefined) {
    style.align_items = source.alignItems as any;
  }
  if (source.flexGrow !== undefined) style.flex_grow = Number(source.flexGrow);
  if (source.flexShrink !== undefined)
    style.flex_shrink = Number(source.flexShrink);
  if (source.gap !== undefined) style.gap = parseLengthUnit(source.gap);

  // Parse colors
  if (source.backgroundColor !== undefined) {
    style.background_color = parseColor(source.backgroundColor);
  }
  if (source.color !== undefined) {
    style.color = parseColor(source.color);
  }
  if (source.borderColor !== undefined) {
    style.border_color = parseColor(source.borderColor);
  }

  // Parse text properties
  if (source.fontSize !== undefined)
    style.font_size = parseLengthUnit(source.fontSize);
  if (source.fontFamily !== undefined) style.font_family = source.fontFamily;
  if (source.fontWeight !== undefined) {
    style.font_weight = source.fontWeight as any;
  }
  if (source.lineHeight !== undefined)
    style.line_height = parseLengthUnit(source.lineHeight);
  if (source.textAlign !== undefined)
    style.text_align = source.textAlign as any;

  // Parse border properties
  if (source.borderRadius !== undefined) {
    style.border_radius = parseSidesValue(source.borderRadius);
  }
  if (source.borderWidth !== undefined) {
    style.border_width = parseSidesValue(source.borderWidth);
  }

  return Object.keys(style).length > 0 ? style : undefined;
}

function parseLengthUnit(value: string | number) {
  if (typeof value === "number") return value;
  if (value === "auto") return "auto";

  const match = value.match(/^(-?[\d.]+)(.*)$/);
  if (!match) return 0;

  const [, num, unit] = match;
  if (!num) return 0;
  const numValue = Number.parseFloat(num);

  switch (unit) {
    case "%":
      return percentage(numValue);
    case "rem":
      return rem(numValue);
    case "em":
      return em(numValue);
    case "vh":
      return vh(numValue);
    case "vw":
      return vw(numValue);
    case "px":
    case "":
      return numValue;
    default:
      return numValue;
  }
}

function parseSidesValue(value: string | number): SidesValue<LengthUnit> {
  if (typeof value === "number") {
    const unit = parseLengthUnit(value);
    return unit;
  }

  const parts = value.trim().split(/\s+/);
  const units = parts.map(parseLengthUnit);

  return units as SidesValue<LengthUnit>;
}

function parseColor(color: string) {
  return color as any;
}
