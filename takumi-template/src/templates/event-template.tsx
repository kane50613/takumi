import type { ReactNode } from "react";

export default function EventTemplate({
  name,
  track,
  datetime,
  location,
  hostName,
  hostTitle,
  accent = "#E5341F",
}: {
  name: ReactNode;
  track: ReactNode;
  datetime: ReactNode;
  location: ReactNode;
  hostName: ReactNode;
  hostTitle: ReactNode;
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
        justifyContent: "space-between",
      }}
    >
      <span
        style={{
          display: "flex",
          fontSize: 20,
          fontWeight: 700,
          letterSpacing: "0.22em",
          textTransform: "uppercase",
          color: accent,
        }}
      >
        {track}
      </span>

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
            fontSize: 88,
            fontWeight: 800,
            lineHeight: 1.05,
            letterSpacing: "-0.035em",
            color: ink,
          }}
        >
          {name}
        </span>
      </div>

      <div
        style={{
          display: "flex",
          alignItems: "flex-end",
          justifyContent: "space-between",
        }}
      >
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <span style={{ display: "flex", fontSize: 28, fontWeight: 600, color: ink }}>
            {datetime}
          </span>
          <span style={{ display: "flex", fontSize: 24, color: muted }}>{location}</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <span style={{ display: "flex", fontSize: 24, fontWeight: 600, color: ink }}>
            {hostName}
          </span>
          <span style={{ display: "flex", fontSize: 24, color: muted }}>·</span>
          <span style={{ display: "flex", fontSize: 24, color: muted }}>{hostTitle}</span>
        </div>
      </div>
    </div>
  );
}
