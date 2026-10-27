import { container, image, text } from "./helpers";
import type { Node, TextNode } from "./types";

export type EmojiType = "twemoji" | "blobmoji" | "noto" | "openmoji" | "fluent" | "fluentFlat";

const ZERO_WIDTH_JOINER = "\u200D";
const TEXT_VARIATION_SELECTOR = "\uFE0E";
const EMOJI_VARIATION_SELECTOR = "\uFE0F";
const EXTENDED_PICTOGRAPHIC_REGEX = /^\p{Extended_Pictographic}/u;
const EMOJI_PRESENTATION_REGEX = /^\p{Emoji_Presentation}/u;
const EMOJI_MODIFIER_SEQUENCE_REGEX = /^\p{Emoji_Modifier_Base}\p{Emoji_Modifier}/u;
const REGIONAL_INDICATOR_REGEX = /^(?:\p{Regional_Indicator}){1,2}$/u;
const KEYCAP_EMOJI_REGEX = /^[#*0-9]\uFE0F?\u20E3$/u;
/** A codepoint every segment {@link isEmojiSegment} accepts holds. */
const EMOJI_CODEPOINT_REGEX = /[\p{Extended_Pictographic}\p{Regional_Indicator}\u20E3]/u;

function getIconCode(char: string) {
  const c = char.includes(ZERO_WIDTH_JOINER) ? char : char.replaceAll(EMOJI_VARIATION_SELECTOR, "");

  return [...c].map((ch) => ch.codePointAt(0)?.toString(16)).join("-");
}

/** Where each provider keeps its SVGs, and the file it names an emoji's code with. */
const providers: Record<EmojiType, { base: string; file: (code: string) => string }> = {
  twemoji: {
    base: "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/",
    file: (code) => `${code.toLowerCase()}.svg`,
  },
  openmoji: {
    base: "https://cdn.jsdelivr.net/npm/@svgmoji/openmoji@2.0.0/svg/",
    file: (code) => `${code.toUpperCase()}.svg`,
  },
  blobmoji: {
    base: "https://cdn.jsdelivr.net/npm/@svgmoji/blob@2.0.0/svg/",
    file: (code) => `${code.toUpperCase()}.svg`,
  },
  noto: {
    base: "https://cdn.jsdelivr.net/gh/googlefonts/noto-emoji@v2.051/svg/",
    file: (code) => `emoji_u${code.toLowerCase().replaceAll("-", "_")}.svg`,
  },
  fluent: {
    base: "https://cdn.jsdelivr.net/gh/shuding/fluentui-emoji-unicode@7a40f1a2d064d76e436813edc0f09b6c8cde5da8/assets/",
    file: (code) => `${code.toLowerCase()}_color.svg`,
  },
  fluentFlat: {
    base: "https://cdn.jsdelivr.net/gh/shuding/fluentui-emoji-unicode@7a40f1a2d064d76e436813edc0f09b6c8cde5da8/assets/",
    file: (code) => `${code.toLowerCase()}_flat.svg`,
  },
};

function getEmojiUrl(icon: string, type: EmojiType) {
  const { base, file } = providers[type];

  return `${base}${file(getIconCode(icon))}`;
}

/**
 * Whether `url` is an emoji SVG one of the providers serves at a stable URL. A URL that parsing
 * would rewrite, such as one with `..` segments, is not one, since it can resolve outside `base`.
 */
export function isEmojiImageUrl(url: string): boolean {
  return (
    URL.canParse(url) &&
    new URL(url).href === url &&
    Object.values(providers).some(({ base }) => url.startsWith(base))
  );
}

const segmenter = new Intl.Segmenter("en", { granularity: "grapheme" });

function getSegments(text: string) {
  return Array.from(segmenter.segment(text));
}

// https://github.com/google/emoji-segmenter/blob/master/emoji_presentation_scanner.rl
function isEmojiSegment(segment: string): boolean {
  if (segment.includes(TEXT_VARIATION_SELECTOR)) {
    return false;
  }

  if (REGIONAL_INDICATOR_REGEX.test(segment) || KEYCAP_EMOJI_REGEX.test(segment)) {
    return true;
  }

  return (
    EXTENDED_PICTOGRAPHIC_REGEX.test(segment) &&
    (segment.includes(EMOJI_VARIATION_SELECTOR) ||
      segment.includes(ZERO_WIDTH_JOINER) ||
      EMOJI_PRESENTATION_REGEX.test(segment) ||
      EMOJI_MODIFIER_SEQUENCE_REGEX.test(segment))
  );
}

function splitTextToNodes(node: TextNode, emojiType: EmojiType): Node[] {
  const nodes: Node[] = [];
  let currentText = "";

  for (const { segment } of getSegments(node.text)) {
    if (isEmojiSegment(segment)) {
      if (currentText) {
        nodes.push(text({ text: currentText }));
        currentText = "";
      }
      nodes.push(
        image({
          src: getEmojiUrl(segment, emojiType),
          style: {
            display: "inline-block",
            width: "1em",
            height: "1em",
            margin: "0 0.05em 0 0.1em",
            verticalAlign: "-0.1em",
          },
        }),
      );
    } else {
      currentText += segment;
    }
  }

  if (currentText) {
    nodes.push(text({ text: currentText }));
  }

  return nodes;
}

/**
 * Replaces each emoji in a node tree with an image node that points at the `emojiType` CDN.
 *
 * @deprecated Pass `emoji` to the renderer, which splits emoji the same way, in HTML too.
 */
export function extractEmojis(node: Node, emojiType: EmojiType): Node {
  if (node.type === "text") {
    const hasEmoji =
      EMOJI_CODEPOINT_REGEX.test(node.text) &&
      getSegments(node.text).some(({ segment }) => isEmojiSegment(segment));

    if (hasEmoji) {
      const { type: _, ...metadata } = node;
      return container({
        ...metadata,
        children: splitTextToNodes(node, emojiType),
      });
    }
  } else if (node.type === "container" && node.children) {
    return {
      ...node,
      children: node.children.map((child) => (child ? extractEmojis(child, emojiType) : child)),
    };
  }

  return node;
}
