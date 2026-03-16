import { describe, expect, it } from "bun:test";
import { extractEmojis } from "../src/emoji";
import { container, image, text } from "../src/helpers";

describe("emoji", () => {
  describe("extractEmojis", () => {
    it("should not modify text without emoji", () => {
      const node = text("Hello, world!");
      const result = extractEmojis(node, "twemoji");
      expect(result).toEqual(node);
    });

    it("should extract simple emoji", () => {
      const content = "Hello 😀";
      const node = text(content);
      const result = extractEmojis(node, "twemoji");

      expect(result).toEqual(
        container({
          children: [
            text("Hello "),
            image({
              src: "https://cdnjs.cloudflare.com/ajax/libs/twemoji/14.0.2/svg/1f600.svg",
              style: {
                display: "inline-block",
                width: "1em",
                height: "1em",
                margin: "0 0.05em 0 0.1em",
                verticalAlign: "-0.1em",
              },
            }),
          ],
        }),
      );
    });

    it("should handle multi-emoji text", () => {
      const content = "😀 transformation 🚀";
      const node = text(content);
      const result = extractEmojis(node, "twemoji");

      expect(result).toEqual(
        container({
          children: [
            image({
              src: "https://cdnjs.cloudflare.com/ajax/libs/twemoji/14.0.2/svg/1f600.svg",
              style: {
                display: "inline-block",
                width: "1em",
                height: "1em",
                margin: "0 0.05em 0 0.1em",
                verticalAlign: "-0.1em",
              },
            }),
            text(" transformation "),
            image({
              src: "https://cdnjs.cloudflare.com/ajax/libs/twemoji/14.0.2/svg/1f680.svg",
              style: {
                display: "inline-block",
                width: "1em",
                height: "1em",
                margin: "0 0.05em 0 0.1em",
                verticalAlign: "-0.1em",
              },
            }),
          ],
        }),
      );
    });

    it("should handle complex emoji (ZWJ)", () => {
      // Family: Man, Woman, Girl, Boy
      const content = "👨‍👩‍👧‍👦";
      const node = text(content);
      const result = extractEmojis(node, "twemoji");

      expect(result).toEqual(
        container({
          children: [
            image({
              src: "https://cdnjs.cloudflare.com/ajax/libs/twemoji/14.0.2/svg/1f468-200d-1f469-200d-1f467-200d-1f466.svg",
              style: {
                display: "inline-block",
                width: "1em",
                height: "1em",
                margin: "0 0.05em 0 0.1em",
                verticalAlign: "-0.1em",
              },
            }),
          ],
        }),
      );
    });

    it("should support different emoji types", () => {
      const emoji = "😀";
      const configs = [
        {
          type: "twemoji",
          expectedSrc:
            "https://cdnjs.cloudflare.com/ajax/libs/twemoji/14.0.2/svg/1f600.svg",
        },
        {
          type: "blobmoji",
          expectedSrc:
            "https://cdn.jsdelivr.net/npm/@svgmoji/blob@2.0.0/svg/1F600.svg",
        },
        {
          type: "noto",
          expectedSrc:
            "https://cdn.jsdelivr.net/gh/svgmoji/svgmoji/packages/svgmoji__noto/svg/1F600.svg",
        },
        {
          type: "openmoji",
          expectedSrc:
            "https://cdn.jsdelivr.net/npm/@svgmoji/openmoji@2.0.0/svg/1F600.svg",
        },
      ] as const;

      for (const config of configs) {
        const node = text(emoji);
        const result = extractEmojis(node, config.type);
        expect(result).toEqual(
          container({
            children: [
              image({
                src: config.expectedSrc,
                style: {
                  display: "inline-block",
                  width: "1em",
                  height: "1em",
                  margin: "0 0.05em 0 0.1em",
                  verticalAlign: "-0.1em",
                },
              }),
            ],
          }),
        );
      }
    });

    it("should preserve metadata when wrapping in container", () => {
      const props = {
        text: "😀",
        id: "my-emoji",
        className: "emoji-class",
        style: { color: "red" },
      };
      const node = text(props);
      const result = extractEmojis(node, "twemoji");

      expect(result).toEqual(
        container({
          id: "my-emoji",
          className: "emoji-class",
          style: { color: "red" },
          children: [
            image({
              src: "https://cdnjs.cloudflare.com/ajax/libs/twemoji/14.0.2/svg/1f600.svg",
              style: {
                display: "inline-block",
                width: "1em",
                height: "1em",
                margin: "0 0.05em 0 0.1em",
                verticalAlign: "-0.1em",
              },
            }),
          ],
        }),
      );
    });

    it("should recursively process container children", () => {
      const node = container({
        children: [
          text("No emoji"),
          container({
            id: "inner",
            children: [text("Nested 😀")],
          }),
        ],
      });

      const result = extractEmojis(node, "twemoji");

      expect(result).toEqual(
        container({
          children: [
            text("No emoji"),
            container({
              id: "inner",
              children: [
                container({
                  children: [
                    text("Nested "),
                    image({
                      src: "https://cdnjs.cloudflare.com/ajax/libs/twemoji/14.0.2/svg/1f600.svg",
                      style: {
                        display: "inline-block",
                        width: "1em",
                        height: "1em",
                        margin: "0 0.05em 0 0.1em",
                        verticalAlign: "-0.1em",
                      },
                    }),
                  ],
                }),
              ],
            }),
          ],
        }),
      );
    });
  });
});
