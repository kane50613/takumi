import type { ReactNode } from "react";

export default function ChangelogTemplate({
  product,
  productInitial = "T",
  version,
  date,
  headline,
  bullets,
  accent = "#1F9D55",
}: {
  product: ReactNode;
  productInitial?: ReactNode;
  version: ReactNode;
  date: ReactNode;
  headline: ReactNode;
  bullets: { tag: ReactNode; text: ReactNode }[];
  accent?: string;
}) {
  const ink = "#16140F";
  const muted = "#6E6A60";
  const hair = "#D7D4CC";
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
        padding: "60px 76px 72px",
        fontFamily: "Inter, sans-serif",
        justifyContent: "space-between",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          paddingBottom: "22px",
          borderBottom: `1px solid ${hair}`,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "18px" }}>
          <div
            style={{
              display: "flex",
              width: 40,
              height: 40,
              borderRadius: "10px",
              backgroundColor: ink,
              color: paper,
              alignItems: "center",
              justifyContent: "center",
              fontSize: 24,
              fontWeight: 800,
            }}
          >
            {productInitial}
          </div>
          <span
            style={{
              display: "flex",
              fontSize: 26,
              fontWeight: 700,
              letterSpacing: "-0.01em",
              color: ink,
            }}
          >
            {product}
          </span>
        </div>
        <span
          style={{
            display: "flex",
            fontSize: 22,
            fontWeight: 600,
            letterSpacing: "0.24em",
            textTransform: "uppercase",
            color: muted,
          }}
        >
          Changelog
        </span>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: "26px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "20px" }}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: "12px",
              backgroundColor: accent,
              color: paper,
              padding: "8px 18px",
              borderRadius: "8px",
              fontSize: 26,
              fontWeight: 700,
              letterSpacing: "-0.01em",
            }}
          >
            {version}
          </div>
          <span
            style={{
              display: "flex",
              fontSize: 25,
              fontWeight: 500,
              color: muted,
            }}
          >
            {date}
          </span>
        </div>

        <h1
          style={{
            display: "flex",
            fontSize: 92,
            fontWeight: 800,
            lineHeight: 1.03,
            letterSpacing: "-0.035em",
            margin: 0,
            maxWidth: "980px",
            color: ink,
          }}
        >
          {headline}
        </h1>
      </div>

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "0px",
          borderTop: `1px solid ${hair}`,
        }}
      >
        {bullets.map((b, i) => (
          <div
            key={i}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "22px",
              paddingTop: "18px",
              paddingBottom: "18px",
              borderBottom: i < bullets.length - 1 ? `1px solid ${hair}` : "none",
            }}
          >
            <div
              style={{
                display: "flex",
                width: 96,
                fontSize: 18,
                fontWeight: 700,
                letterSpacing: "0.12em",
                textTransform: "uppercase",
                color: accent,
              }}
            >
              {b.tag}
            </div>
            <span
              style={{
                display: "flex",
                fontSize: 30,
                fontWeight: 500,
                color: ink,
              }}
            >
              {b.text}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
