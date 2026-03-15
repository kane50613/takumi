import { describe, expect, it } from "bun:test";
import { Renderer } from "../index.js";

const renderer = new Renderer();

describe("CSS style parsing", () => {
  it("parses a broad set of container layout and visual styles", async () => {
    const result = await renderer.measure(
      {
        type: "container",
        style: {
          width: 320,
          height: 180,
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
          alignItems: "stretch",
          gap: "12px",
          padding: "16px 20px",
          margin: "4px",
          backgroundColor: "#111827",
          color: "white",
          border: "2px solid #374151",
          borderRadius: "12px / 8px",
          opacity: 0.95,
          overflow: "hidden",
          position: "relative",
          transform: "translateX(8px) translateY(4px) scale(1)",
          transformOrigin: "0 0",
          boxShadow: "0 8px 24px rgba(0,0,0,0.25)",
          backgroundImage:
            "linear-gradient(to bottom right, rgba(255,255,255,0.08), transparent)",
        },
        children: [
          {
            type: "container",
            style: {
              width: "100%",
              height: 48,
              backgroundColor: "rgba(255,255,255,0.08)",
              borderRadius: 8,
            },
          },
          {
            type: "container",
            style: {
              width: "60%",
              height: 24,
              backgroundColor: "#60a5fa",
              opacity: 0.8,
            },
          },
        ],
      },
      { width: 320, height: 180 },
    );

    expect(result.width).toBe(320);
    expect(result.height).toBe(180);
    expect(result.children).toHaveLength(2);
  });

  it("parses text truncation and typography styles", async () => {
    const result = await renderer.measure(
      {
        type: "container",
        style: {
          width: 360,
          padding: "12px",
          backgroundColor: "#f3f4f6",
        },
        children: [
          {
            type: "text",
            text: "This is a long text node used to validate CSS parsing for typography and truncation related style properties.",
            style: {
              width: "100%",
              display: "flex",
              fontSize: "20px",
              fontWeight: "700",
              lineHeight: 1.3,
              letterSpacing: "-0.02em",
              color: "#111827",
              textAlign: "left",
              textTransform: "none",
              textDecoration: "underline",
              textDecorationThickness: "2px",
              textShadow: "0 1px 2px rgba(0,0,0,0.15)",
              textOverflow: "ellipsis",
              lineClamp: "1",
              overflow: "hidden",
              wordBreak: "break-all",
              textWrap: "pretty",
            },
          },
        ],
      },
      { width: 360 },
    );

    expect(result.width).toBe(360);
    expect(result.children).toHaveLength(1);
  });

  it("parses additional sizing and positioning values as strings", async () => {
    const result = await renderer.measure(
      {
        type: "container",
        style: {
          width: 400,
          height: 220,
          position: "relative",
          backgroundColor: "white",
          padding: "8px",
        },
        children: [
          {
            type: "container",
            style: {
              position: "absolute",
              top: "10px",
              right: "12px",
              width: "50%",
              minHeight: "32px",
              maxWidth: "200px",
              backgroundColor: "tomato",
              borderRadius: "9999px",
            },
          },
          {
            type: "container",
            style: {
              width: "100%",
              height: "100%",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            },
            children: [
              {
                type: "text",
                text: "ok",
                style: {
                  fontSize: 16,
                  color: "black",
                },
              },
            ],
          },
        ],
      },
      { width: 400, height: 220 },
    );

    expect(result.width).toBe(400);
    expect(result.height).toBe(220);
  });
});
