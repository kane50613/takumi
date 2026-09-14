import { afterAll, describe, expect, it } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
import { Renderer } from "../bundlers/node";

describe("Renderer.measure", () => {
  const renderer = new Renderer();

  afterAll(() => renderer.free());

  it("should measure a simple container", async () => {
    const node = container({
      style: {
        width: 100,
        height: 100,
        backgroundColor: "red",
      },
      children: [],
    });

    const result = await renderer.measure(node);

    expect(result).toEqual({
      width: 100,
      height: 100,
      transform: [1, 0, 0, 1, 0, 0],
      children: [],
      runs: [],
    });
  });

  it("should measure nested children with layout", async () => {
    const node = container({
      style: {
        display: "flex",
        width: 200,
        height: 200,
        padding: 10,
      },
      children: [
        text({
          text: "Hello",
          style: {
            display: "flex",
            width: 50,
            height: 50,
          },
        }),
        container({
          style: {
            flex: 1,
            height: 50,
          },
        }),
      ],
    });

    const result = await renderer.measure(node);

    expect(result).toMatchObject({
      width: 200,
      height: 200,
      transform: [1, 0, 0, 1, 0, 0],
      runs: [],
    });

    expect(result.children).toHaveLength(2);
    expect(result.children[0]).toMatchObject({
      width: 50,
      height: 50,
      transform: [1, 0, 0, 1, 10, 10],
      runs: [],
    });
    expect(result.children[1]).toMatchObject({
      width: 130,
      height: 50,
      transform: [1, 0, 0, 1, 60, 10],
      children: [],
      runs: [],
    });
    expect(result.children[0]?.children).toHaveLength(1);
    expect(result.children[0]?.children[0]).toMatchObject({
      height: 50,
      transform: [1, 0, 0, 1, 10, 10],
      children: [],
    });
  });

  it("should omit styles unless includeStyles is set", async () => {
    const node = container({
      style: { width: 100, height: 100, backgroundColor: "red" },
      children: [],
    });

    const result = await renderer.measure(node);

    expect(result).not.toHaveProperty("style");
  });

  it("should return resolved styles when includeStyles is set", async () => {
    const node = container({
      style: {
        width: 100,
        height: 100,
        backgroundColor: "red",
        borderRadius: 8,
        opacity: 0.5,
      },
      children: [
        text({
          text: "Hello",
          style: { fontSize: 20, color: "#14110f" },
        }),
      ],
    });

    const result = await renderer.measure(node, { includeStyles: true });

    expect(result.style).toMatchObject({
      backgroundColor: "rgb(255, 0, 0)",
      borderRadius: [8, 8, 8, 8],
      opacity: 0.5,
    });
    expect(result.runs[0]?.style).toMatchObject({
      fontSize: 20,
      color: "rgb(20, 17, 15)",
      fontWeight: 400,
      fontStyle: "normal",
      letterSpacing: 0,
    });
  });
});
