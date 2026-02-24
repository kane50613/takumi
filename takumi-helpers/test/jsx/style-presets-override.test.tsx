import { describe, expect, test } from "bun:test";
import type { CSSProperties } from "react";
import { fromJsx } from "../../src/jsx/jsx";
import { defaultStylePresets } from "../../src/jsx/style-presets";
import type { ContainerNode, TextNode } from "../../src/types";

describe("fromJsx - stylePresets overriding", () => {
  describe("default behavior", () => {
    test("applies default style presets to h1 element", async () => {
      const { node } = await fromJsx(<h1>Hello</h1>);
      expect(node).toEqual({
        type: "text",
        text: "Hello",
        preset: defaultStylePresets.h1,
        tagName: "h1",
      } satisfies TextNode);
    });

    test("applies default style presets to p element", async () => {
      const { node } = await fromJsx(<p>Paragraph</p>);
      expect(node).toEqual({
        type: "text",
        text: "Paragraph",
        preset: defaultStylePresets.p,
        tagName: "p",
      } satisfies TextNode);
    });

    test("applies default style presets to strong element", async () => {
      const { node } = await fromJsx(<strong>Bold</strong>);
      expect(node).toEqual({
        type: "text",
        text: "Bold",
        preset: defaultStylePresets.strong,
        tagName: "strong",
      } satisfies TextNode);
    });

    test("applies default style presets to span for raw text", async () => {
      const { node } = await fromJsx("Plain text");
      expect(node).toEqual({
        type: "text",
        text: "Plain text",
        preset: defaultStylePresets.span,
      } satisfies TextNode);
    });
  });

  describe("disabling default styles with false", () => {
    test("disables default styles for h1 element", async () => {
      const { node } = await fromJsx(<h1>Hello</h1>, { defaultStyles: false });
      expect(node).toEqual({
        type: "text",
        text: "Hello",
        tagName: "h1",
      } satisfies TextNode);
    });

    test("disables default styles for p element", async () => {
      const { node } = await fromJsx(<p>Paragraph</p>, {
        defaultStyles: false,
      });
      expect(node).toEqual({
        type: "text",
        text: "Paragraph",
        tagName: "p",
      } satisfies TextNode);
    });

    test("disables default styles for strong element", async () => {
      const { node } = await fromJsx(<strong>Bold</strong>, {
        defaultStyles: false,
      });
      expect(node).toEqual({
        type: "text",
        text: "Bold",
        tagName: "strong",
      } satisfies TextNode);
    });

    test("disables default styles for raw text", async () => {
      const { node } = await fromJsx("Plain text", { defaultStyles: false });
      expect(node).toEqual({
        type: "text",
        text: "Plain text",
      } satisfies TextNode);
    });

    test("disables default styles for img element", async () => {
      const { node } = await fromJsx(
        <img src="https://example.com/image.jpg" alt="Test" />,
        { defaultStyles: false },
      );
      expect(node).toEqual({
        type: "image",
        src: "https://example.com/image.jpg",
        width: undefined,
        height: undefined,
        tagName: "img",
      });
    });

    test("disables default styles for nested elements", async () => {
      const { node } = await fromJsx(
        <div>
          <h1>Title</h1>
          <p>
            Text with <strong>bold</strong>
          </p>
        </div>,
        { defaultStyles: false },
      );

      expect(node).toEqual({
        type: "container",
        children: [
          {
            type: "text",
            text: "Title",
            tagName: "h1",
          },
          {
            type: "container",
            tagName: "p",
            children: [
              {
                type: "text",
                text: "Text with ",
              },
              {
                type: "text",
                text: "bold",
                tagName: "strong",
              },
            ],
          },
        ],
        tagName: "div",
      } satisfies ContainerNode);
    });
  });

  describe("custom style presets", () => {
    test("overrides h1 preset with custom styles", async () => {
      const customPresets = {
        ...defaultStylePresets,
        h1: {
          fontSize: "3em",
          color: "red",
          fontWeight: "normal",
        } as CSSProperties,
      };

      const { node } = await fromJsx(<h1>Custom</h1>, {
        defaultStyles: customPresets,
      });

      expect(node).toEqual({
        type: "text",
        text: "Custom",
        preset: {
          fontSize: "3em",
          color: "red",
          fontWeight: "normal",
        },
        tagName: "h1",
      } satisfies TextNode);
    });

    test("overrides p preset with custom styles", async () => {
      const customPresets = {
        ...defaultStylePresets,
        p: {
          marginTop: "2em",
          marginBottom: "2em",
          color: "blue",
        } as CSSProperties,
      };

      const { node } = await fromJsx(<p>Custom paragraph</p>, {
        defaultStyles: customPresets,
      });

      expect(node).toEqual({
        type: "text",
        text: "Custom paragraph",
        preset: {
          marginTop: "2em",
          marginBottom: "2em",
          color: "blue",
        },
        tagName: "p",
      } satisfies TextNode);
    });

    test("overrides multiple presets", async () => {
      const customPresets = {
        ...defaultStylePresets,
        h1: {
          fontSize: "4em",
          color: "purple",
        } as CSSProperties,
        strong: {
          fontWeight: "900",
          color: "orange",
        } as CSSProperties,
      };

      const { node } = await fromJsx(
        <div>
          <h1>Title</h1>
          <strong>Bold</strong>
        </div>,
        { defaultStyles: customPresets },
      );

      expect(node).toEqual({
        type: "container",
        children: [
          {
            type: "text",
            text: "Title",
            preset: {
              fontSize: "4em",
              color: "purple",
            },
            tagName: "h1",
          },
          {
            type: "text",
            text: "Bold",
            preset: {
              fontWeight: "900",
              color: "orange",
            },
            tagName: "strong",
          },
        ],
        tagName: "div",
      } satisfies ContainerNode);
    });

    test("adds new preset for custom element", async () => {
      const customPresets = {
        ...defaultStylePresets,
        article: {
          padding: "20px",
          backgroundColor: "#f0f0f0",
        } as CSSProperties,
      };

      const { node } = await fromJsx(<article>Article content</article>, {
        defaultStyles: customPresets,
      });

      expect(node).toEqual({
        type: "text",
        text: "Article content",
        preset: {
          padding: "20px",
          backgroundColor: "#f0f0f0",
        },
        tagName: "article",
      } satisfies TextNode);
    });

    test("partial override keeps unspecified presets as default", async () => {
      const customPresets = {
        ...defaultStylePresets,
        h1: {
          fontSize: "5em",
        } as CSSProperties,
      };

      const { node } = await fromJsx(
        <div>
          <h1>Custom H1</h1>
          <h2>Default H2</h2>
        </div>,
        { defaultStyles: customPresets },
      );

      expect(node).toEqual({
        type: "container",
        children: [
          {
            type: "text",
            text: "Custom H1",
            preset: {
              fontSize: "5em",
            },
            tagName: "h1",
          },
          {
            type: "text",
            text: "Default H2",
            preset: defaultStylePresets.h2,
            tagName: "h2",
          },
        ],
        tagName: "div",
      } satisfies ContainerNode);
    });
  });

  describe("inline styles override presets", () => {
    test("inline styles override default presets", async () => {
      const { node } = await fromJsx(
        <h1 style={{ fontSize: "10em", color: "green" }}>Inline styled</h1>,
      );

      expect(node).toEqual({
        type: "text",
        text: "Inline styled",
        preset: defaultStylePresets.h1,
        style: {
          fontSize: "10em",
          color: "green",
        },
        tagName: "h1",
      } satisfies TextNode);
    });

    test("inline styles override custom presets", async () => {
      const customPresets = {
        ...defaultStylePresets,
        h1: {
          fontSize: "3em",
          color: "red",
        } as CSSProperties,
      };

      const { node } = await fromJsx(
        <h1 style={{ fontSize: "10em", fontWeight: "100" }}>
          Inline override
        </h1>,
        { defaultStyles: customPresets },
      );

      expect(node).toEqual({
        type: "text",
        text: "Inline override",
        preset: {
          fontSize: "3em",
          color: "red",
        },
        style: {
          fontSize: "10em",
          fontWeight: "100",
        },
        tagName: "h1",
      } satisfies TextNode);
    });

    test("inline styles work when default styles are disabled", async () => {
      const { node } = await fromJsx(
        <h1 style={{ fontSize: "8em", color: "blue" }}>No presets</h1>,
        { defaultStyles: false },
      );

      expect(node).toEqual({
        type: "text",
        text: "No presets",
        style: {
          fontSize: "8em",
          color: "blue",
        },
        tagName: "h1",
      } satisfies TextNode);
    });
  });

  describe("complex scenarios", () => {
    test("deeply nested elements with custom presets", async () => {
      const customPresets = {
        ...defaultStylePresets,
        h1: { fontSize: "4em" } as CSSProperties,
        p: { marginTop: "2em" } as CSSProperties,
        strong: { fontWeight: "900" } as CSSProperties,
      };

      const { node } = await fromJsx(
        <div>
          <h1>Title</h1>
          <div>
            <p>
              Paragraph with <strong>bold</strong> text
            </p>
          </div>
        </div>,
        { defaultStyles: customPresets },
      );

      expect(node).toEqual({
        type: "container",
        children: [
          {
            type: "text",
            text: "Title",
            preset: customPresets.h1,
            tagName: "h1",
          },
          {
            type: "container",
            tagName: "div",
            children: [
              {
                type: "container",
                tagName: "p",
                children: [
                  {
                    type: "text",
                    text: "Paragraph with ",
                    preset: defaultStylePresets.span,
                  },
                  {
                    type: "text",
                    text: "bold",
                    preset: customPresets.strong,
                    tagName: "strong",
                  },
                  {
                    type: "text",
                    text: " text",
                    preset: defaultStylePresets.span,
                  },
                ],
                preset: customPresets.p,
              },
            ],
          },
        ],
        tagName: "div",
      } satisfies ContainerNode);
    });

    test("mixed inline styles and custom presets", async () => {
      const customPresets = {
        ...defaultStylePresets,
        h1: { fontSize: "3em", color: "red" } as CSSProperties,
        p: { marginTop: "1.5em" } as CSSProperties,
      };

      const { node } = await fromJsx(
        <div>
          <h1 style={{ color: "blue" }}>Styled Title</h1>
          <p>Normal paragraph</p>
          <p style={{ color: "green" }}>Green paragraph</p>
        </div>,
        { defaultStyles: customPresets },
      );

      expect(node).toEqual({
        type: "container",
        children: [
          {
            type: "text",
            text: "Styled Title",
            preset: {
              fontSize: "3em",
              color: "red",
            },
            style: {
              color: "blue",
            },
            tagName: "h1",
          },
          {
            type: "text",
            text: "Normal paragraph",
            preset: {
              marginTop: "1.5em",
            },
            tagName: "p",
          },
          {
            type: "text",
            text: "Green paragraph",
            preset: {
              marginTop: "1.5em",
            },
            style: {
              color: "green",
            },
            tagName: "p",
          },
        ],
        tagName: "div",
      } satisfies ContainerNode);
    });

    test("function component with custom presets", async () => {
      const MyComponent = ({ title }: { title: string }) => (
        <div>
          <h1>{title}</h1>
          <p>Content</p>
        </div>
      );

      const customPresets = {
        ...defaultStylePresets,
        h1: { fontSize: "5em" } as CSSProperties,
      };

      const { node } = await fromJsx(<MyComponent title="Test" />, {
        defaultStyles: customPresets,
      });

      expect(node).toEqual({
        type: "container",
        children: [
          {
            type: "text",
            text: "Test",
            preset: customPresets.h1,
            tagName: "h1",
          },
          {
            type: "text",
            text: "Content",
            preset: defaultStylePresets.p,
            tagName: "p",
          },
        ],
        tagName: "div",
      } satisfies ContainerNode);
    });

    test("empty custom presets object (no presets)", async () => {
      const { node } = await fromJsx(
        <div>
          <h1>Title</h1>
          <p>Paragraph</p>
        </div>,
        { defaultStyles: {} },
      );

      expect(node).toEqual({
        type: "container",
        children: [
          {
            type: "text",
            text: "Title",
            tagName: "h1",
          },
          {
            type: "text",
            text: "Paragraph",
            tagName: "p",
          },
        ],
        tagName: "div",
      } satisfies ContainerNode);
    });
  });

  describe("edge cases", () => {
    test("undefined defaultStyles option uses default presets", async () => {
      const { node } = await fromJsx(<h1>Hello</h1>, {
        defaultStyles: undefined,
      });
      expect(node).toEqual({
        type: "text",
        text: "Hello",
        preset: defaultStylePresets.h1,
        tagName: "h1",
      } satisfies TextNode);
    });

    test("empty options object uses default presets", async () => {
      const { node } = await fromJsx(<h1>Hello</h1>, {});
      expect(node).toEqual({
        type: "text",
        text: "Hello",
        preset: defaultStylePresets.h1,
        tagName: "h1",
      } satisfies TextNode);
    });

    test("custom presets with only one element defined", async () => {
      const customPresets = {
        h1: { fontSize: "6em" } as CSSProperties,
      };

      const { node } = await fromJsx(
        <div>
          <h1>Custom</h1>
          <p>Default</p>
        </div>,
        { defaultStyles: customPresets },
      );

      expect(node).toEqual({
        type: "container",
        children: [
          {
            type: "text",
            text: "Custom",
            preset: customPresets.h1,
            tagName: "h1",
          },
          {
            type: "text",
            text: "Default",
            tagName: "p",
          },
        ],
        tagName: "div",
      } satisfies ContainerNode);
    });

    test("custom presets are passed through nested function components", async () => {
      const Inner = () => <h1>Inner Title</h1>;
      const Outer = () => (
        <div>
          <Inner />
        </div>
      );

      const customPresets = {
        ...defaultStylePresets,
        h1: { fontSize: "7em" } as CSSProperties,
      };

      const { node } = await fromJsx(<Outer />, {
        defaultStyles: customPresets,
      });

      expect(node).toEqual({
        type: "container",
        children: [
          {
            type: "text",
            text: "Inner Title",
            preset: customPresets.h1,
            tagName: "h1",
          },
        ],
        tagName: "div",
      } satisfies ContainerNode);
    });
  });
});
