import type { ReactNode } from "react";

const Stat = ({ value, label }: { value: ReactNode; label: ReactNode }) => (
  <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
    <span style={{ display: "flex", fontSize: 28, fontWeight: 700, color: "#16140F" }}>
      {value}
    </span>
    <span style={{ display: "flex", fontSize: 28, color: "#6E6A60" }}>{label}</span>
  </div>
);

export default function RepositoryTemplate({
  owner,
  name,
  description,
  stars,
  forks,
  language,
  langColor,
}: {
  owner: ReactNode;
  name: ReactNode;
  description: ReactNode;
  stars: ReactNode;
  forks: ReactNode;
  language: ReactNode;
  langColor: string;
}) {
  const ink = "#16140F";
  const muted = "#6E6A60";
  const mono = "'Geist Mono', monospace";
  const sans = "Geist, sans-serif";

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
        fontFamily: sans,
        justifyContent: "center",
      }}
    >
      <span
        style={{
          display: "flex",
          fontSize: 40,
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
          maxWidth: 920,
          marginTop: 32,
        }}
      >
        {description}
      </span>

      <div style={{ display: "flex", alignItems: "center", gap: 16, marginTop: 44 }}>
        <Stat value={stars} label="stars" />
        <span style={{ display: "flex", fontSize: 28, color: muted }}>·</span>
        <Stat value={forks} label="forks" />
        <span style={{ display: "flex", fontSize: 28, color: muted }}>·</span>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <div
            style={{
              display: "flex",
              width: 20,
              height: 20,
              borderRadius: "50%",
              backgroundColor: langColor,
            }}
          />
          <span style={{ display: "flex", fontSize: 28, fontWeight: 600, color: ink }}>
            {language}
          </span>
        </div>
      </div>
    </div>
  );
}
