import { paint } from "takumi-paint";

export const tree = await paint(
  <div style={{ display: "flex", padding: 24, background: "#F7F3EC" }}>
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 10,
        width: 272,
        padding: 20,
        background: "white",
        border: "2px solid #E7E0D4",
        borderRadius: 16,
        boxShadow: "0 8px 20px rgba(60, 40, 20, 0.16)",
      }}
    >
      <div
        style={{
          width: 48,
          height: 48,
          borderRadius: 24,
          overflow: "hidden",
          background: "linear-gradient(135deg, #8B5CF6, #F472B6)",
        }}
      />
      <b style={{ fontSize: 22 }}>Hello, paint</b>
      <span style={{ opacity: 0.6, fontSize: 14 }}>Boxes, text, and steps</span>
    </div>
  </div>,
  { width: 368 },
);
