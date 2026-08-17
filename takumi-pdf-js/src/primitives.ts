import type { ReactElementLike } from "@takumi-rs/helpers";

/**
 * A `@counter-style` name the page counters can format in, matching the set
 * the renderer's `@counter-style` substitution knows.
 */
export type CounterStyle =
  | "decimal"
  | "decimal-leading-zero"
  | "lower-roman"
  | "upper-roman"
  | "lower-alpha"
  | "lower-latin"
  | "upper-alpha"
  | "upper-latin"
  | "lower-greek"
  | "hiragana"
  | "katakana"
  | "trad-chinese-informal"
  | "cjk-ideographic"
  | "cjk-decimal"
  | "arabic-indic"
  | "bengali"
  | "cambodian"
  | "khmer"
  | "devanagari"
  | "gujarati"
  | "gurmukhi"
  | "kannada"
  | "lao"
  | "malayalam"
  | "mongolian"
  | "myanmar"
  | "oriya"
  | "persian"
  | "urdu"
  | "tamil"
  | "telugu"
  | "thai"
  | "tibetan";

/** Props shared by the page counter primitives. */
export type CounterProps = {
  /** Counter style the number formats in. Defaults to decimal. */
  format?: CounterStyle;
  className?: string;
  style?: Record<string, unknown>;
  tw?: string;
};

// Marks the returned objects as JSX elements, so the node/JSX input split
// routes them through `fromJsx` instead of reading them as takumi nodes.
const ELEMENT = Symbol.for("react.transitional.element");

function counter(hook: string, { format, className, ...rest }: CounterProps): ReactElementLike {
  return {
    $$typeof: ELEMENT,
    type: "span",
    props: {
      ...rest,
      className: [hook, format, className].filter(Boolean).join(" "),
    },
  };
}

/**
 * The number of the page this element lands on. Meaningful inside a `header`
 * or `footer` band (or a `position: fixed` box), where it renumbers per page.
 */
export function PageNumber(props: CounterProps = {}): ReactElementLike {
  return counter("pageNumber", props);
}

/** The document's total page count. Same placement rules as {@link PageNumber}. */
export function TotalPages(props: CounterProps = {}): ReactElementLike {
  return counter("totalPages", props);
}

/**
 * The number of the page `href`'s target element lands on, for cross
 * references like "see page 12". Rendered as a link to the target.
 */
export function TargetPageNumber({
  href,
  ...props
}: CounterProps & { href: string }): ReactElementLike {
  const span = counter("targetPageNumber", props);

  return { $$typeof: ELEMENT, type: "a", props: { href, children: span } };
}
