import { ImageResponse } from "@takumi-rs/image-response";

export function GET() {
  return new ImageResponse(
    <div
      style={{
        alignItems: "center",
        background: "linear-gradient(120deg, #111827, #1f2937)",
        color: "#f9fafb",
        display: "flex",
        flexDirection: "column",
        fontFamily: "system-ui, sans-serif",
        gap: 16,
        height: "100%",
        justifyContent: "center",
        width: "100%",
      }}
    >
      <div style={{ fontSize: 88, fontWeight: 700 }}>Waku SSR</div>
      <div style={{ fontSize: 40, opacity: 0.9 }}>Generated with Takumi ImageResponse</div>
    </div>,
    {
      width: 1200,
      height: 630,
    },
  );
}

export const getConfig = async () => {
  return {
    render: "dynamic",
  } as const;
};
