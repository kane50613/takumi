import { describe, expect, test } from "bun:test";
import { User2 } from "lucide-react";
import { renderToStaticMarkup } from "react-dom/server";
import { container } from "../../src/helpers";
import { fromJsx } from "../../src/jsx/jsx";
import { defaultStylePresets } from "../../src/jsx/style-presets";
import type { ContainerNode, ImageNode, TextNode } from "../../src/types";

describe("fromJsx", () => {
  test("handles React like object", async () => {
    const result = await fromJsx({
      type: "div",
      props: {
        children: "Hello World",
      },
    });

    expect(result).toEqual({
      type: "text",
      text: "Hello World",
      tagName: "div",
    } satisfies TextNode);
  });

  test("converts text to TextNode", async () => {
    const result = await fromJsx("Hello World");
    expect(result).toEqual({
      type: "text",
      text: "Hello World",
      preset: defaultStylePresets.span,
    } satisfies TextNode);
  });

  test("converts number to TextNode", async () => {
    const result = await fromJsx(42);
    expect(result).toEqual({
      type: "text",
      text: "42",
      preset: defaultStylePresets.span,
    } satisfies TextNode);
  });

  test("returns empty container for null/undefined/false", async () => {
    expect(await fromJsx(null)).toEqual({
      type: "container",
    } satisfies ContainerNode);
    expect(await fromJsx(undefined)).toEqual({
      type: "container",
    } satisfies ContainerNode);
    expect(await fromJsx(false)).toEqual({
      type: "container",
    } satisfies ContainerNode);
  });

  test("converts simple div to ContainerNode", async () => {
    const result = await fromJsx(<div>Hello</div>);
    expect(result).toEqual({
      type: "text",
      text: "Hello",
      tagName: "div",
    } satisfies TextNode);
  });

  test("passes tagName, id, className to text nodes", async () => {
    const result = await fromJsx(
      <p id="headline" className="text-xl">
        Hello
      </p>,
    );

    expect(result).toEqual({
      type: "text",
      text: "Hello",
      preset: defaultStylePresets.p,
      tagName: "p",
      id: "headline",
      className: "text-xl",
    } satisfies TextNode);
  });

  test("passes tagName, id, className to container nodes", async () => {
    const result = await fromJsx(
      <div id="wrapper" className="stack">
        <span>First</span>
        <span>Second</span>
      </div>,
    );

    expect(result).toEqual({
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
      tagName: "div",
      id: "wrapper",
      className: "stack",
    } satisfies ContainerNode);
  });

  test("handles function components", async () => {
    const MyComponent = ({ name }: { name: string }) => <div>Hello {name}</div>;

    const result = await fromJsx(<MyComponent name="World" />);
    expect(result).toEqual({
      type: "text",
      text: "Hello World",
      tagName: "div",
    } satisfies TextNode);
  });

  test("handles style casing correctly", async () => {
    const result = await fromJsx(
      <p
        style={{
          WebkitTextStroke: "1px red",
        }}
      >
        Hello
      </p>,
    );

    expect(result).toEqual({
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

    const result = await fromJsx(<AsyncComponent name="Async" />);
    expect(result).toEqual({
      type: "text",
      text: "Hello Async",
      tagName: "div",
    } satisfies TextNode);
  });

  test("handles fragments", async () => {
    const result = await fromJsx(
      <>
        <div>First</div>
        <div>Second</div>
      </>,
    );

    expect(result).toEqual({
      type: "container",
      children: [
        { type: "text", text: "First", tagName: "div" },
        { type: "text", text: "Second", tagName: "div" },
      ],
      style: {
        width: "100%",
        height: "100%",
      },
    } satisfies ContainerNode);
  });

  test("handles arrays", async () => {
    const items = ["First", "Second", "Third"];
    const result = await fromJsx(
      <div>
        {items.map((item) => (
          <span key={item}>{item}</span>
        ))}
      </div>,
    );

    expect(result).toEqual({
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
      tagName: "div",
    } satisfies ContainerNode);
  });

  test("treats nested array children as non-pure text", async () => {
    const result = await fromJsx({
      type: "p",
      props: {
        children: ["Hello", [" World"]],
      },
    });

    expect(result).toEqual({
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
    const result = await fromJsx({
      type: "p",
      props: {
        children: ["Hello", null],
      },
    });

    expect(result).toEqual({
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
    const result = await fromJsx(
      <img src="https://example.com/image.jpg" alt="Test" />,
    );
    expect(result).toEqual({
      type: "image",
      src: "https://example.com/image.jpg",
      width: undefined,
      height: undefined,
      preset: defaultStylePresets.img,
      tagName: "img",
    } satisfies ImageNode);
  });

  test("passes tagName, id, className to img nodes", async () => {
    const result = await fromJsx(
      <img
        src="https://example.com/image.jpg"
        id="hero-image"
        className="rounded"
        alt="Test"
      />,
    );

    expect(result).toEqual({
      type: "image",
      src: "https://example.com/image.jpg",
      width: undefined,
      height: undefined,
      preset: defaultStylePresets.img,
      tagName: "img",
      id: "hero-image",
      className: "rounded",
    } satisfies ImageNode);
  });

  test("converts img elements with width and height to ImageNode", async () => {
    const result = await fromJsx(
      <img
        src="https://example.com/image.jpg"
        width={100}
        height={100}
        alt="Test"
      />,
    );
    expect(result).toEqual({
      type: "image",
      src: "https://example.com/image.jpg",
      width: 100,
      height: 100,
      preset: defaultStylePresets.img,
      tagName: "img",
    } satisfies ImageNode);
  });

  test("maps default tw property to node tw", async () => {
    const result = await fromJsx(<p tw="text-red-500">Hello</p>);

    expect(result).toEqual({
      type: "text",
      text: "Hello",
      preset: defaultStylePresets.p,
      tw: "text-red-500",
      tagName: "p",
    } satisfies TextNode);
  });

  test("maps configured tailwind classes property to node tw", async () => {
    const result = await fromJsx(
      {
        type: "p",
        props: {
          children: "Hello",
          classes: "text-red-500",
        },
      },
      { tailwindClassesProperty: "classes" },
    );

    expect(result).toEqual({
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
    expect((await fromJsx(<User2 />)).type).toBe("image");
  });

  test("handles deeply nested structures", async () => {
    const result = await fromJsx(
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

    expect(result).toEqual({
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
      tagName: "div",
    } satisfies ContainerNode);
  });

  test("handles promises", async () => {
    const promiseElement = Promise.resolve("Resolved text");
    const result = await fromJsx(promiseElement);
    expect(result).toEqual({
      type: "text",
      text: "Resolved text",
      preset: defaultStylePresets.span,
    } satisfies TextNode);
  });

  test("integration: fromJsx result as container children with complex JSX", async () => {
    // Test complex JSX structure that can be directly used as container children
    const complexJsx = await fromJsx(
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
      children: [complexJsx],
    });

    expect(complexContainer).toEqual({
      type: "container",
      children: [
        {
          type: "container",
          tagName: "div",
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

    const result = await fromJsx(component);
    expect(result).toEqual({
      type: "image",
      src: renderToStaticMarkup(component),
      width: 60,
      height: 60,
      preset: defaultStylePresets.svg,
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
    const result = await fromJsx(component);

    expect(result).toEqual({
      type: "image",
      src: renderToStaticMarkup(component),
      width: 10,
      height: 12,
      preset: defaultStylePresets.svg,
      tagName: "svg",
      id: "logo",
      className: "icon",
    } satisfies ImageNode);
  });

  test("passes tagName, id, className to br text nodes", async () => {
    const result = await fromJsx(<br id="line-break" className="spacer" />);

    expect(result).toEqual({
      type: "text",
      text: "\n",
      preset: defaultStylePresets.span,
      tagName: "br",
      id: "line-break",
      className: "spacer",
    } satisfies TextNode);
  });
});
