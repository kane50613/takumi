import { describe, expect, test } from "bun:test";
import { User2 } from "lucide-react";
import type { ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { container } from "../../src/helpers";
import { fromJsx } from "../../src/jsx/jsx";
import { defaultStylePresets } from "../../src/jsx/style-presets";
import type { ContainerNode, ImageNode, TextNode } from "../../src/types";

describe("fromJsx", () => {
  test("handles React like object", async () => {
    const { node } = await fromJsx({
      type: "div",
      props: {
        children: "Hello World",
      },
    });

    expect(node).toEqual({
      type: "text",
      text: "Hello World",
      preset: defaultStylePresets.div,
      tagName: "div",
    } satisfies TextNode);
  });

  test("converts text to TextNode", async () => {
    const { node } = await fromJsx("Hello World");
    expect(node).toEqual({
      type: "text",
      text: "Hello World",
      preset: defaultStylePresets.span,
    } satisfies TextNode);
  });

  test("converts number to TextNode", async () => {
    const { node } = await fromJsx(42);
    expect(node).toEqual({
      type: "text",
      text: "42",
      preset: defaultStylePresets.span,
    } satisfies TextNode);
  });

  test("returns empty container for null/undefined/false", async () => {
    {
      const { node } = await fromJsx(null);
      expect(node).toEqual({
        type: "container",
      } satisfies ContainerNode);
    }
    {
      const { node } = await fromJsx(undefined);
      expect(node).toEqual({
        type: "container",
      } satisfies ContainerNode);
    }
    {
      const { node } = await fromJsx(false);
      expect(node).toEqual({
        type: "container",
      } satisfies ContainerNode);
    }
  });

  test("converts simple div to ContainerNode", async () => {
    const { node } = await fromJsx(<div>Hello</div>);
    expect(node).toEqual({
      type: "text",
      text: "Hello",
      preset: defaultStylePresets.div,
      tagName: "div",
    } satisfies TextNode);
  });

  test("passes tagName, id, className to text nodes", async () => {
    const { node } = await fromJsx(
      <p id="headline" className="text-xl">
        Hello
      </p>,
    );

    expect(node).toEqual({
      type: "text",
      text: "Hello",
      preset: defaultStylePresets.p,
      tagName: "p",
      id: "headline",
      className: "text-xl",
    } satisfies TextNode);
  });

  test("passes tagName, id, className to container nodes", async () => {
    const { node } = await fromJsx(
      <div id="wrapper" className="stack">
        <span>First</span>
        <span>Second</span>
      </div>,
    );

    expect(node).toEqual({
      type: "container",
      children: [
        {
          type: "text",
          text: "First",
          preset: defaultStylePresets.span,
          tagName: "span",
        },
        {
          type: "text",
          text: "Second",
          preset: defaultStylePresets.span,
          tagName: "span",
        },
      ],
      preset: defaultStylePresets.div,
      tagName: "div",
      id: "wrapper",
      className: "stack",
    } satisfies ContainerNode);
  });

  test("handles function components", async () => {
    const MyComponent = ({ name }: { name: string }) => <div>Hello {name}</div>;

    const { node } = await fromJsx(<MyComponent name="World" />);
    expect(node).toEqual({
      type: "text",
      text: "Hello World",
      preset: defaultStylePresets.div,
      tagName: "div",
    } satisfies TextNode);
  });

  test("handles style casing correctly", async () => {
    const { node } = await fromJsx(
      <p
        style={{
          WebkitTextStroke: "1px red",
        }}
      >
        Hello
      </p>,
    );

    expect(node).toEqual({
      type: "text",
      text: "Hello",
      preset: {
        marginTop: "1em",
        marginBottom: "1em",
        display: "block",
      },
      style: {
        WebkitTextStroke: "1px red",
      },
      tagName: "p",
    } satisfies TextNode);
  });

  test("handles async function components", async () => {
    const AsyncComponent = async ({ name }: { name: string }) => (
      <div>Hello {name}</div>
    );

    const { node } = await fromJsx(<AsyncComponent name="Async" />);
    expect(node).toEqual({
      type: "text",
      text: "Hello Async",
      preset: defaultStylePresets.div,
      tagName: "div",
    } satisfies TextNode);
  });

  test("handles fragments", async () => {
    const { node } = await fromJsx(
      <>
        <div>First</div>
        <div>Second</div>
      </>,
    );

    expect(node).toEqual({
      type: "container",
      children: [
        {
          type: "text",
          text: "First",
          tagName: "div",
          preset: defaultStylePresets.div,
        },
        {
          type: "text",
          text: "Second",
          tagName: "div",
          preset: defaultStylePresets.div,
        },
      ],
      style: {
        width: "100%",
        height: "100%",
      },
    } satisfies ContainerNode);
  });

  test("handles arrays", async () => {
    const items = ["First", "Second", "Third"];
    const { node } = await fromJsx(
      <div>
        {items.map((item) => (
          <span key={item}>{item}</span>
        ))}
      </div>,
    );

    expect(node).toEqual({
      type: "container",
      children: [
        {
          type: "text",
          text: "First",
          preset: defaultStylePresets.span,
          tagName: "span",
        },
        {
          type: "text",
          text: "Second",
          preset: defaultStylePresets.span,
          tagName: "span",
        },
        {
          type: "text",
          text: "Third",
          preset: defaultStylePresets.span,
          tagName: "span",
        },
      ],
      preset: defaultStylePresets.div,
      tagName: "div",
    } satisfies ContainerNode);
  });

  test("treats nested array children as non-pure text", async () => {
    const { node } = await fromJsx({
      type: "p",
      props: {
        children: ["Hello", [" World"]],
      },
    });

    expect(node).toEqual({
      type: "container",
      preset: defaultStylePresets.p,
      tagName: "p",
      children: [
        {
          type: "text",
          text: "Hello",
          preset: defaultStylePresets.span,
        },
        {
          type: "text",
          text: " World",
          preset: defaultStylePresets.span,
        },
      ],
    } satisfies ContainerNode);
  });

  test("treats null children in iterables as non-pure text", async () => {
    const { node } = await fromJsx({
      type: "p",
      props: {
        children: ["Hello", null],
      },
    });

    expect(node).toEqual({
      type: "container",
      preset: defaultStylePresets.p,
      tagName: "p",
      children: [
        {
          type: "text",
          text: "Hello",
          preset: defaultStylePresets.span,
        },
      ],
    } satisfies ContainerNode);
  });

  test("converts img elements to ImageNode", async () => {
    const { node } = await fromJsx(
      <img src="https://example.com/image.jpg" alt="Test" />,
    );
    expect(node).toEqual({
      type: "image",
      src: "https://example.com/image.jpg",
      width: undefined,
      height: undefined,
      preset: defaultStylePresets.img,
      attributes: {
        src: "https://example.com/image.jpg",
        alt: "Test",
      },
      tagName: "img",
    } satisfies ImageNode);
  });

  test("passes tagName, id, className to img nodes", async () => {
    const { node } = await fromJsx(
      <img
        src="https://example.com/image.jpg"
        id="hero-image"
        className="rounded"
        alt="Test"
      />,
    );

    expect(node).toEqual({
      type: "image",
      src: "https://example.com/image.jpg",
      width: undefined,
      height: undefined,
      preset: defaultStylePresets.img,
      attributes: {
        src: "https://example.com/image.jpg",
        alt: "Test",
      },
      tagName: "img",
      id: "hero-image",
      className: "rounded",
    } satisfies ImageNode);
  });

  test("converts img elements with width and height to ImageNode", async () => {
    const { node } = await fromJsx(
      <img
        src="https://example.com/image.jpg"
        width={100}
        height={100}
        alt="Test"
      />,
    );
    expect(node).toEqual({
      type: "image",
      src: "https://example.com/image.jpg",
      width: 100,
      height: 100,
      preset: defaultStylePresets.img,
      attributes: {
        src: "https://example.com/image.jpg",
        width: "100",
        height: "100",
        alt: "Test",
      },
      tagName: "img",
    } satisfies ImageNode);
  });

  test("maps default tw property to node tw", async () => {
    const { node } = await fromJsx(<p tw="text-red-500">Hello</p>);

    expect(node).toEqual({
      type: "text",
      text: "Hello",
      preset: defaultStylePresets.p,
      tw: "text-red-500",
      tagName: "p",
    } satisfies TextNode);
  });

  test("maps configured tailwind classes property to node tw", async () => {
    const { node } = await fromJsx(
      {
        type: "p",
        props: {
          children: "Hello",
          classes: "text-red-500",
        },
      },
      { tailwindClassesProperty: "classes" },
    );

    expect(node).toEqual({
      type: "text",
      text: "Hello",
      preset: defaultStylePresets.p,
      tw: "text-red-500",
      tagName: "p",
    } satisfies TextNode);
  });

  test("handles img without src satisfies container", () => {
    expect(fromJsx(<img alt="No src" />)).rejects.toThrowError(
      "Image element must have a 'src' prop.",
    );
  });

  test("handles external lucide-react icon", async () => {
    const { node } = await fromJsx(<User2 />);
    expect(node.type).toBe("image");
  });

  test("handles deeply nested structures", async () => {
    const { node } = await fromJsx(
      <div>
        <h1>Title</h1>
        <div>
          <p>
            Paragraph with <strong>bold</strong> text
          </p>
          <ul>
            <li>Item 1</li>
            <li>Item 2</li>
          </ul>
        </div>
      </div>,
    );

    expect(node).toEqual({
      type: "container",
      children: [
        {
          type: "text",
          text: "Title",
          preset: defaultStylePresets.h1,
          tagName: "h1",
        },
        {
          type: "container",
          tagName: "div",
          preset: defaultStylePresets.div,
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
                  preset: defaultStylePresets.strong,
                  tagName: "strong",
                },
                {
                  type: "text",
                  text: " text",
                  preset: defaultStylePresets.span,
                },
              ],
              preset: defaultStylePresets.p,
            },
            {
              type: "container",
              tagName: "ul",
              children: [
                {
                  type: "text",
                  text: "Item 1",
                  tagName: "li",
                },
                {
                  type: "text",
                  text: "Item 2",
                  tagName: "li",
                },
              ],
            },
          ],
        },
      ],
      preset: defaultStylePresets.div,
      tagName: "div",
    } satisfies ContainerNode);
  });

  test("handles promises", async () => {
    const promiseElement = Promise.resolve("Resolved text");
    const { node } = await fromJsx(promiseElement);
    expect(node).toEqual({
      type: "text",
      text: "Resolved text",
      preset: defaultStylePresets.span,
    } satisfies TextNode);
  });

  test("integration: fromJsx result as container children with complex JSX", async () => {
    // Test complex JSX structure that can be directly used as container children
    const { node } = await fromJsx(
      <div>
        <h1>Welcome</h1>
        <div>
          <span>Item 1</span>
          <span>Item 2</span>
        </div>
        <img src="https://example.com/logo.png" alt="Logo" />
      </div>,
    );

    const complexContainer = container({
      children: [node],
    });

    expect(complexContainer).toEqual({
      type: "container",
      children: [
        {
          type: "container",
          tagName: "div",
          preset: defaultStylePresets.div,
          children: [
            {
              type: "text",
              text: "Welcome",
              preset: defaultStylePresets.h1,
              tagName: "h1",
            },
            {
              type: "container",
              tagName: "div",
              preset: defaultStylePresets.div,
              children: [
                {
                  type: "text",
                  text: "Item 1",
                  preset: defaultStylePresets.span,
                  tagName: "span",
                },
                {
                  type: "text",
                  text: "Item 2",
                  preset: defaultStylePresets.span,
                  tagName: "span",
                },
              ],
            },
            {
              type: "image",
              src: "https://example.com/logo.png",
              width: undefined,
              height: undefined,
              preset: defaultStylePresets.img,
              attributes: {
                src: "https://example.com/logo.png",
                alt: "Logo",
              },
              tagName: "img",
            },
          ],
        },
      ],
    } satisfies ContainerNode);
  });

  test("handles svg elements", async () => {
    const component = (
      <svg
        width="60"
        height="60"
        viewBox="0 0 180 180"
        filter="url(#logo-shadow)"
        xmlns="http://www.w3.org/2000/svg"
      >
        <title>Logo</title>
        <circle cx="90" cy="90" r="86" fill="url(#logo-iconGradient)" />
        <defs>
          <filter id="logo-shadow" colorInterpolationFilters="sRGB">
            <feDropShadow
              dx="0"
              dy="0"
              stdDeviation="4"
              floodColor="white"
              floodOpacity="1"
            />
          </filter>
          <linearGradient id="logo-iconGradient" gradientTransform="rotate(45)">
            <stop offset="45%" stopColor="black" />
            <stop offset="100%" stopColor="white" />
          </linearGradient>
        </defs>
      </svg>
    );

    const { node } = await fromJsx(component);
    expect(node).toEqual({
      type: "image",
      src: renderToStaticMarkup(component),
      width: 60,
      height: 60,
      preset: defaultStylePresets.svg,
      attributes: {
        width: "60",
        height: "60",
        viewBox: "0 0 180 180",
        filter: "url(#logo-shadow)",
        xmlns: "http://www.w3.org/2000/svg",
      },
      tagName: "svg",
    });
  });

  test("passes tagName, id, className to svg nodes", async () => {
    const component = (
      <svg
        id="logo"
        className="icon"
        width="10"
        height="12"
        xmlns="http://www.w3.org/2000/svg"
      >
        <title>Logo</title>
        <rect width="10" height="12" />
      </svg>
    );
    const { node } = await fromJsx(component);

    expect(node).toEqual({
      type: "image",
      src: renderToStaticMarkup(component),
      width: 10,
      height: 12,
      preset: defaultStylePresets.svg,
      attributes: {
        width: "10",
        height: "12",
        xmlns: "http://www.w3.org/2000/svg",
      },
      tagName: "svg",
      id: "logo",
      className: "icon",
    } satisfies ImageNode);
  });

  test("passes tagName, id, className to br text nodes", async () => {
    const { node } = await fromJsx(<br id="line-break" className="spacer" />);

    expect(node).toEqual({
      type: "text",
      text: "\n",
      preset: defaultStylePresets.span,
      tagName: "br",
      id: "line-break",
      className: "spacer",
    } satisfies TextNode);
  });

  test("collects JSX attributes into node metadata", async () => {
    const { node } = await fromJsx(
      <button
        type="button"
        data-kind="hero"
        aria-label="Promo"
        draggable
        hidden={false}
      >
        <img src="https://example.com/a.png" alt="Preview" draggable />
      </button>,
    );

    expect(node).toMatchObject({
      type: "container",
      tagName: "button",
      attributes: {
        type: "button",
        "data-kind": "hero",
        "aria-label": "Promo",
        draggable: "",
      },
      children: [
        {
          type: "image",
          src: "https://example.com/a.png",
          tagName: "img",
          attributes: {
            alt: "Preview",
            draggable: "",
            src: "https://example.com/a.png",
          },
        },
      ],
    } satisfies ContainerNode);
  });

  test("extracts style tag contents into stylesheets", async () => {
    const { node, stylesheets } = await fromJsx(
      <div>
        <style>{".box { color: red; }"}</style>
        <span>Hello</span>
      </div>,
    );

    expect(stylesheets).toEqual([".box { color: red; }"]);
    expect(node).toEqual({
      type: "container",
      tagName: "div",
      preset: defaultStylePresets.div,
      children: [
        {
          type: "text",
          text: "Hello",
          preset: defaultStylePresets.span,
          tagName: "span",
        },
      ],
    } satisfies ContainerNode);
  });

  test("extracts stylesheets from fragments and preserves order", async () => {
    const Wrapper = ({ children }: { children: ReactNode }) => <>{children}</>;

    const { node, stylesheets } = await fromJsx(
      <div>
        <Wrapper>
          <style>{".a { color: red; }"}</style>
        </Wrapper>
        <style>{".b { color: blue; }"}</style>
        <span>Content</span>
      </div>,
    );

    expect(stylesheets).toEqual([".a { color: red; }", ".b { color: blue; }"]);
    expect(node).toEqual({
      type: "container",
      tagName: "div",
      preset: defaultStylePresets.div,
      children: [
        {
          type: "text",
          text: "Content",
          preset: defaultStylePresets.span,
          tagName: "span",
        },
      ],
    } satisfies ContainerNode);
  });

  test("ignores boolean children while extracting style text", async () => {
    const { stylesheets } = await fromJsx(
      <style>
        {"body{"}
        {true}
        {"color:red;}"}
      </style>,
    );

    expect(stylesheets).toEqual(["body{color:red;}"]);
  });
});
