import { container, image, text } from "./helpers";
import type { Node, TextNode } from "./types";

export type EmojiType = "twemoji" | "blobmoji" | "noto" | "openmoji" | "fluent" | "fluentFlat";

const UFE0Fg = /\uFE0F/g;
const U200D = String.fromCharCode(0x200d);
const EXTENDED_PICTOGRAPHIC_REGEX = /\p{Extended_Pictographic}/u;
const REGIONAL_INDICATOR_PAIR_REGEX = /^(?:\p{Regional_Indicator}){2}$/u;
const KEYCAP_EMOJI_REGEX = /^[#*0-9]\uFE0F?\u20E3$/u;

function getIconCode(char: string) {
  const c = char.indexOf(U200D) < 0 ? char.replace(UFE0Fg, "") : char;
  let r = "";
  for (let i = 0, p = 0; i < c.length; i++) {
    const cc = c.charCodeAt(i);
    if (p) {
      const code = (65536 + ((p - 55296) << 10) + (cc - 56320)).toString(16);
      r += (r ? "-" : "") + code;
      p = 0;
    } else if (55296 <= cc && cc <= 56319) {
      p = cc;
    } else {
      r += (r ? "-" : "") + cc.toString(16);
    }
  }
  return r;
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

let segmenter: Intl.Segmenter | null | undefined;

function getSegmenter(): Intl.Segmenter | null {
  if (segmenter === undefined) {
    if (typeof Intl !== "undefined" && typeof Intl.Segmenter === "function") {
      segmenter = new Intl.Segmenter("en", { granularity: "grapheme" });
    } else {
      segmenter = null;
    }
  }
  return segmenter;
}

function getSegments(text: string): { segment: string }[] {
  const s = getSegmenter();
  if (s) {
    return Array.from(s.segment(text));
  }
  return Array.from(text).map((s) => ({ segment: s }));
}

function isEmojiSegment(segment: string): boolean {
  return (
    EXTENDED_PICTOGRAPHIC_REGEX.test(segment) ||
    REGIONAL_INDICATOR_PAIR_REGEX.test(segment) ||
    KEYCAP_EMOJI_REGEX.test(segment)
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
