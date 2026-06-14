import type { ReactNode } from "react";

export default function ChangelogTemplate({
  version,
  date,
  headline,
  bullets,
  accent = "#1F9D55",
}: {
  version: ReactNode;
  date: ReactNode;
  headline: ReactNode;
  bullets: { tag: ReactNode; text: ReactNode }[];
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
      <div style={{ display: "flex", alignItems: "center", gap: 16, marginBottom: 24 }}>
        <span style={{ display: "flex", fontSize: 28, fontWeight: 700, color: accent }}>
          {version}
        </span>
        <span style={{ display: "flex", fontSize: 28, color: muted }}>·</span>
        <span style={{ display: "flex", fontSize: 28, color: muted }}>{date}</span>
      </div>

      <h1
        style={{
          display: "flex",
          fontSize: 72,
          fontWeight: 800,
          lineHeight: 1.05,
          letterSpacing: "-0.03em",
          margin: 0,
          marginBottom: 56,
          maxWidth: 980,
          color: ink,
        }}
      >
        {headline}
      </h1>

      <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
        {bullets.map((b, i) => (
          <div key={i} style={{ display: "flex", alignItems: "center", gap: 28 }}>
            <div
              style={{
                display: "flex",
                width: 92,
                fontSize: 20,
                fontWeight: 700,
                letterSpacing: "0.12em",
                textTransform: "uppercase",
                color: accent,
              }}
            >
              {b.tag}
            </div>
            <span style={{ display: "flex", fontSize: 32, fontWeight: 500, color: ink }}>
              {b.text}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
