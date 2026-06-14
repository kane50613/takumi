import type { ReactNode } from "react";

export default function QuoteTemplate({
  quote,
  author,
  role,
  company,
  initials,
  eyebrow = "Customer Story",
  brand = "Takumi",
  accent = "#E5341F",
}: {
  quote: ReactNode;
  author: ReactNode;
  role: ReactNode;
  company: ReactNode;
  initials: ReactNode;
  eyebrow?: ReactNode;
  brand?: ReactNode;
  accent?: string;
}) {
  const ink = "#16140F";
  const muted = "#6E6A60";
  const paper = "#F5F3EC";

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        backgroundColor: paper,
        color: ink,
        padding: "68px 76px",
        fontFamily: "Inter, sans-serif",
        justifyContent: "space-between",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          paddingBottom: 24,
          borderBottom: `2px solid ${ink}`,
        }}
      >
        <span
          style={{
            display: "flex",
            fontSize: 20,
            fontWeight: 700,
            letterSpacing: "0.22em",
            textTransform: "uppercase",
            color: ink,
          }}
        >
          {eyebrow}
        </span>
        <span
          style={{
            display: "flex",
            fontSize: 20,
            fontWeight: 600,
            letterSpacing: "0.04em",
            color: muted,
          }}
        >
          {brand}
        </span>
      </div>

      <div
        style={{
          display: "flex",
          flex: 1,
          flexDirection: "column",
          justifyContent: "center",
        }}
      >
        <span
          style={{
            display: "flex",
            height: 88,
            marginBottom: 8,
            marginLeft: -8,
            fontSize: 200,
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
            maxWidth: 1040,
            color: ink,
          }}
        >
          {quote}
        </h1>
      </div>

      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 24,
          paddingTop: 28,
          borderTop: `2px solid ${ink}`,
        }}
      >
        <div
          style={{
            display: "flex",
            width: 76,
            height: 76,
            borderRadius: "50%",
            backgroundColor: ink,
            color: paper,
            alignItems: "center",
            justifyContent: "center",
            fontSize: 32,
            fontWeight: 700,
            letterSpacing: "0.01em",
          }}
        >
          {initials}
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <span style={{ display: "flex", fontSize: 32, fontWeight: 700, color: ink }}>
            {author}
          </span>
          <span style={{ display: "flex", fontSize: 24, fontWeight: 500, color: muted }}>
            {role}
          </span>
        </div>
        <div style={{ display: "flex", flex: 1 }} />
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 16,
          }}
        >
          <div
            style={{
              display: "flex",
              width: 16,
              height: 16,
              borderRadius: "50%",
              backgroundColor: accent,
            }}
          />
          <span style={{ display: "flex", fontSize: 28, fontWeight: 700, color: ink }}>
            {company}
          </span>
        </div>
      </div>
    </div>
  );
}
