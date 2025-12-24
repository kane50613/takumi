import type { ReactNode } from "react";

export default function BlogPostTemplate({
  title,
  author,
  date,
  category,
  avatar,
  backgroundImage,
}: {
  title: ReactNode;
  author: ReactNode;
  date: ReactNode;
  category: ReactNode;
  avatar?: ReactNode;
  backgroundImage?: string;
}) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        backgroundColor: "#111",
        color: "white",
        backgroundImage:
          backgroundImage || "linear-gradient(135deg, #1a1a1a 0%, #000 100%)",
        padding: "60px",
        justifyContent: "space-between",
      }}
    >
      <div style={{ display: "flex", alignItems: "flex-start" }}>
        <div
          style={{
            backgroundColor: "#3b82f6",
            color: "white",
            padding: "8px 24px",
            borderRadius: "9999px",
            fontSize: 24,
            fontWeight: 600,
          }}
        >
          {category}
        </div>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: "24px" }}>
        <h1
          style={{
            fontSize: 80,
            fontWeight: 800,
            lineHeight: 1.1,
            margin: 0,
            textShadow: "0 4px 12px rgba(0,0,0,0.5)",
          }}
        >
          {title}
        </h1>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: "24px" }}>
        {avatar && (
          <div
            style={{
              width: 80,
              height: 80,
              borderRadius: "50%",
              overflow: "hidden",
              border: "4px solid rgba(255,255,255,0.1)",
            }}
          >
            {avatar}
          </div>
        )}
        <div style={{ display: "flex", flexDirection: "column" }}>
          <span style={{ fontSize: 32, fontWeight: 600 }}>{author}</span>
          <span style={{ fontSize: 24, color: "#a1a1aa" }}>{date}</span>
        </div>
      </div>
    </div>
  );
}
