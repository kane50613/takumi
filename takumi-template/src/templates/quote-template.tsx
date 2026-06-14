import type { ReactNode } from "react";

export default function QuoteTemplate({
  quote,
  author,
  role,
  company,
  accent = "#E5341F",
}: {
  quote: ReactNode;
  author: ReactNode;
  role: ReactNode;
  company: ReactNode;
  accent?: string;
}) {
  const ink = "#16140F";
  const muted = "#6E6A60";

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        backgroundColor: "#F5F3EC",
        color: ink,
        padding: "80px 76px",
        fontFamily: "Inter, sans-serif",
        justifyContent: "center",
      }}
    >
      <span
        style={{
          display: "flex",
          height: 72,
          marginLeft: -8,
          marginBottom: 8,
          fontSize: 180,
          lineHeight: 1,
          fontWeight: 700,
          color: accent,
          fontFamily: "Georgia, serif",
        }}
      >
        {"“"}
      </span>
      <h1
        style={{
          display: "flex",
          fontSize: 64,
          fontWeight: 700,
          lineHeight: 1.12,
          letterSpacing: "-0.02em",
          margin: 0,
          marginBottom: 48,
          maxWidth: 1000,
          color: ink,
        }}
      >
        {quote}
      </h1>
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <span style={{ display: "flex", fontSize: 28, fontWeight: 700, color: ink }}>{author}</span>
        <span style={{ display: "flex", fontSize: 28, color: muted }}>·</span>
        <span style={{ display: "flex", fontSize: 28, color: muted }}>{role}</span>
        <span style={{ display: "flex", fontSize: 28, color: muted }}>·</span>
        <span style={{ display: "flex", fontSize: 28, color: muted }}>{company}</span>
      </div>
    </div>
  );
}
