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
        padding: "64px 76px",
        fontFamily: "Inter, sans-serif",
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
        <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
          <div
            style={{
              display: "flex",
              width: 44,
              height: 44,
              borderRadius: 12,
              backgroundColor: ink,
              color: paper,
              alignItems: "center",
              justifyContent: "center",
              fontSize: 28,
              fontWeight: 800,
            }}
          >
            {productInitial}
          </div>
          <span
            style={{
              display: "flex",
              fontSize: 28,
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
            fontSize: 20,
            fontWeight: 700,
            letterSpacing: "0.24em",
            textTransform: "uppercase",
            color: muted,
          }}
        >
          Changelog
        </span>
      </div>

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          flex: 1,
          justifyContent: "space-between",
          paddingTop: 40,
        }}
      >
        <div style={{ display: "flex", flexDirection: "column" }}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 20,
              marginBottom: 24,
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                backgroundColor: accent,
                color: paper,
                padding: "8px 16px",
                borderRadius: 8,
                fontSize: 28,
                fontWeight: 700,
                letterSpacing: "-0.01em",
              }}
            >
              {version}
            </div>
            <span
              style={{
                display: "flex",
                fontSize: 24,
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
              fontSize: 64,
              fontWeight: 800,
              lineHeight: 1.06,
              letterSpacing: "-0.03em",
              margin: 0,
              maxWidth: 980,
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
            borderTop: `1px solid ${hair}`,
          }}
        >
          {bullets.map((b, i) => (
            <div
              key={i}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 28,
                paddingTop: 16,
                paddingBottom: 16,
                borderBottom: i < bullets.length - 1 ? `1px solid ${hair}` : "none",
              }}
            >
              <div
                style={{
                  display: "flex",
                  width: 100,
                  fontSize: 20,
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
                  fontSize: 32,
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
    </div>
  );
}
