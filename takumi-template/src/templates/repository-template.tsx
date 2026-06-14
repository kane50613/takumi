import type { ReactNode } from "react";

export default function RepositoryTemplate({
  owner,
  name,
  description,
  stars,
  forks,
  language,
  langColor,
  accent = "#E5341F",
}: {
  owner: ReactNode;
  name: ReactNode;
  description: ReactNode;
  stars: ReactNode;
  forks: ReactNode;
  language: ReactNode;
  langColor: string;
  accent?: string;
}) {
  const bg = "#F5F3EC";
  const ink = "#16140F";
  const muted = "#6E6A60";
  const hair = "#D7D4CC";
  const mono = "'Geist Mono', monospace";
  const sans = "Geist, sans-serif";

  const StatBlock = ({
    label,
    children,
    divider,
  }: {
    label: ReactNode;
    children: ReactNode;
    divider?: boolean;
  }) => (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 12,
        paddingRight: 56,
        marginRight: 56,
        borderRight: divider ? `1px solid ${hair}` : "none",
      }}
    >
      <span
        style={{
          display: "flex",
          fontSize: 20,
          fontWeight: 600,
          letterSpacing: "0.2em",
          textTransform: "uppercase",
          color: muted,
          fontFamily: sans,
        }}
      >
        {label}
      </span>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 16,
          fontSize: 48,
          fontWeight: 700,
          color: ink,
          fontFamily: sans,
          letterSpacing: "-0.01em",
        }}
      >
        {children}
      </div>
    </div>
  );

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        backgroundColor: bg,
        color: ink,
        borderTop: `8px solid ${accent}`,
        padding: "56px 76px 64px",
        fontFamily: sans,
        justifyContent: "space-between",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
          <div
            style={{
              display: "flex",
              width: 48,
              height: 48,
              borderRadius: 12,
              backgroundColor: ink,
              color: bg,
              alignItems: "center",
              justifyContent: "center",
              fontSize: 32,
              fontWeight: 700,
              fontFamily: mono,
            }}
          >
            /
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
            GitHub
          </span>
        </div>
        <span
          style={{
            display: "flex",
            fontSize: 20,
            fontWeight: 600,
            letterSpacing: "0.22em",
            textTransform: "uppercase",
            color: muted,
          }}
        >
          Public Repository
        </span>
      </div>

      <div
        style={{
          display: "flex",
          flex: 1,
          flexDirection: "column",
          justifyContent: "center",
          paddingTop: 28,
        }}
      >
        <span
          style={{
            display: "flex",
            fontSize: 44,
            fontWeight: 500,
            fontFamily: mono,
            color: muted,
            letterSpacing: "-0.01em",
          }}
        >
          {owner}/
        </span>
        <span
          style={{
            display: "flex",
            fontSize: 132,
            fontWeight: 700,
            fontFamily: mono,
            color: ink,
            lineHeight: 0.96,
            letterSpacing: "-0.045em",
            marginTop: 4,
          }}
        >
          {name}
        </span>
        <span
          style={{
            display: "flex",
            fontSize: 32,
            fontWeight: 400,
            color: "#37352F",
            lineHeight: 1.36,
            maxWidth: 900,
            marginTop: 32,
          }}
        >
          {description}
        </span>
      </div>

      <div
        style={{
          display: "flex",
          alignItems: "flex-start",
          paddingTop: 32,
          borderTop: `1px solid ${hair}`,
        }}
      >
        <StatBlock label="Stars" divider>
          {stars}
        </StatBlock>
        <StatBlock label="Forks" divider>
          {forks}
        </StatBlock>
        <StatBlock label="Language">
          <div
            style={{
              display: "flex",
              width: 24,
              height: 24,
              borderRadius: "50%",
              backgroundColor: langColor,
            }}
          />
          {language}
        </StatBlock>
      </div>
    </div>
  );
}
