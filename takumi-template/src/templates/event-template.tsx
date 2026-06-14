import type { ReactNode } from "react";

export default function EventTemplate({
  name,
  track,
  datetime,
  location,
  online = false,
  hostName,
  hostTitle,
  hostInitials,
  brand = "TAKUMI",
  accent = "#E5341F",
}: {
  name: ReactNode;
  track: ReactNode;
  datetime: ReactNode;
  location: ReactNode;
  online?: boolean;
  hostName: ReactNode;
  hostTitle: ReactNode;
  hostInitials: ReactNode;
  brand?: ReactNode;
  accent?: string;
}) {
  const ink = "#16140F";
  const muted = "#6E6A60";
  const hair = "#D7D4CC";

  const metaLabel = {
    display: "flex",
    fontSize: 17,
    fontWeight: 700,
    letterSpacing: "0.18em",
    textTransform: "uppercase" as const,
    color: muted,
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        backgroundColor: "#F5F3EC",
        color: ink,
        padding: "72px 76px",
        fontFamily: "Inter, sans-serif",
        justifyContent: "space-between",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          paddingBottom: "26px",
          borderBottom: `2px solid ${ink}`,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "16px" }}>
          <div
            style={{
              display: "flex",
              width: 14,
              height: 14,
              backgroundColor: accent,
            }}
          />
          <span
            style={{
              display: "flex",
              fontSize: 22,
              fontWeight: 700,
              letterSpacing: "0.22em",
              textTransform: "uppercase",
              color: accent,
            }}
          >
            {track}
          </span>
        </div>
        <span
          style={{
            display: "flex",
            fontSize: 22,
            fontWeight: 800,
            letterSpacing: "0.3em",
            color: ink,
          }}
        >
          {brand}
        </span>
      </div>

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          flex: 1,
          justifyContent: "center",
          paddingTop: "20px",
          paddingBottom: "20px",
        }}
      >
        <span
          style={{
            display: "flex",
            fontSize: 84,
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
          paddingTop: "30px",
          borderTop: `1px solid ${hair}`,
        }}
      >
        <div style={{ display: "flex", gap: "64px" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
            <span style={metaLabel}>When</span>
            <span
              style={{
                display: "flex",
                fontSize: 27,
                fontWeight: 600,
                color: ink,
              }}
            >
              {datetime}
            </span>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
            <span style={metaLabel}>Where</span>
            {online ? (
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "10px",
                  backgroundColor: accent,
                  padding: "5px 16px 7px",
                  alignSelf: "flex-start",
                }}
              >
                <div
                  style={{
                    display: "flex",
                    width: 10,
                    height: 10,
                    borderRadius: "50%",
                    backgroundColor: "#FFFFFF",
                  }}
                />
                <span
                  style={{
                    display: "flex",
                    fontSize: 25,
                    fontWeight: 700,
                    letterSpacing: "0.04em",
                    color: "#FFFFFF",
                  }}
                >
                  {location}
                </span>
              </div>
            ) : (
              <span
                style={{
                  display: "flex",
                  fontSize: 27,
                  fontWeight: 600,
                  color: ink,
                }}
              >
                {location}
              </span>
            )}
          </div>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: "18px" }}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              width: 60,
              height: 60,
              borderRadius: "50%",
              border: `2px solid ${ink}`,
              fontSize: 23,
              fontWeight: 700,
              color: ink,
            }}
          >
            {hostInitials}
          </div>
          <div style={{ display: "flex", flexDirection: "column" }}>
            <span
              style={{
                display: "flex",
                fontSize: 26,
                fontWeight: 700,
                color: ink,
              }}
            >
              {hostName}
            </span>
            <span style={{ display: "flex", fontSize: 21, color: muted }}>{hostTitle}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
