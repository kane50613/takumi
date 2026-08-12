import { container, image, text } from "./helpers";
import type { Node, TextNode } from "./types";

export type EmojiType = "twemoji" | "blobmoji" | "noto" | "openmoji" | "fluent" | "fluentFlat";

const UFE0Fg = /\uFE0F/g;
const U200D = String.fromCharCode(0x200d);
const TEXT_VARIATION_SELECTOR = "\uFE0E";
const EMOJI_VARIATION_SELECTOR = "\uFE0F";
const EXTENDED_PICTOGRAPHIC_REGEX = /^\p{Extended_Pictographic}/u;
const EMOJI_PRESENTATION_REGEX = /^\p{Emoji_Presentation}/u;
const EMOJI_MODIFIER_SEQUENCE_REGEX = /^\p{Emoji_Modifier_Base}\p{Emoji_Modifier}/u;
const REGIONAL_INDICATOR_REGEX = /^(?:\p{Regional_Indicator}){1,2}$/u;
const KEYCAP_EMOJI_REGEX = /^[#*0-9]\uFE0F?\u20E3$/u;

function getIconCode(char: string) {
  const c = char.indexOf(U200D) < 0 ? char.replace(UFE0Fg, "") : char;

  return [...c].map((ch) => ch.codePointAt(0)?.toString(16)).join("-");
}

const apis = {
  twemoji: (code: string) =>
    `https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/${code.toLowerCase()}.svg`,
  openmoji: "https://cdn.jsdelivr.net/npm/@svgmoji/openmoji@2.0.0/svg/",
  blobmoji: "https://cdn.jsdelivr.net/npm/@svgmoji/blob@2.0.0/svg/",
  noto: (code: string) =>
    `https://cdn.jsdelivr.net/gh/googlefonts/noto-emoji@v2.051/svg/emoji_u${code.toLowerCase().replaceAll("-", "_")}.svg`,
  fluent: (code: string) =>
    `https://cdn.jsdelivr.net/gh/shuding/fluentui-emoji-unicode/assets/${code.toLowerCase()}_color.svg`,
  fluentFlat: (code: string) =>
    `https://cdn.jsdelivr.net/gh/shuding/fluentui-emoji-unicode/assets/${code.toLowerCase()}_flat.svg`,
};

function getEmojiUrl(icon: string, type: EmojiType) {
  const code = getIconCode(icon);
  const api = apis[type];
  return typeof api === "function" ? api(code) : `${api}${code.toUpperCase()}.svg`;
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
      segment.includes(U200D) ||
      EMOJI_PRESENTATION_REGEX.test(segment) ||
      EMOJI_MODIFIER_SEQUENCE_REGEX.test(segment))
  );
}

function splitTextToNodes(node: TextNode, emojiType: EmojiType): Node[] {
  const nodes: Node[] = [];
  let currentText = "";

  const segments = getSegments(node.text);

  for (const { segment } of segments) {
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

export function extractEmojis(node: Node, emojiType: EmojiType): Node {
  if (node.type === "text") {
    const hasEmoji = getSegments(node.text).some(({ segment }) => isEmojiSegment(segment));

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
