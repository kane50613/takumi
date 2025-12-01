import type { ReactNode } from "react";

export default function SocialQuoteTemplateV1({
  quote,
  author,
  role,
  avatar,
}: {
  quote: ReactNode;
  author: ReactNode;
  role?: ReactNode;
  avatar?: ReactNode;
}) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        background: "linear-gradient(135deg, #6366f1 0%, #a855f7 100%)",
        color: "white",
        padding: "80px",
        justifyContent: "center",
        alignItems: "center",
        textAlign: "center",
      }}
    >
      <div
        style={{
          fontSize: 120,
          lineHeight: 1,
          fontFamily: "serif",
          opacity: 0.5,
          marginBottom: "-40px",
        }}
      >
        “
      </div>
      <blockquote
        style={{
          fontSize: 64,
          fontWeight: 700,
          lineHeight: 1.3,
          margin: "0 0 60px 0",
          maxWidth: "90%",
        }}
      >
        {quote}
      </blockquote>
      <div style={{ display: "flex", alignItems: "center", gap: "24px" }}>
        {avatar && (
          <div
            style={{
              width: 96,
              height: 96,
              borderRadius: "50%",
              overflow: "hidden",
              border: "4px solid rgba(255,255,255,0.3)",
            }}
          >
            {avatar}
          </div>
        )}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "flex-start",
          }}
        >
          <span style={{ fontSize: 36, fontWeight: 700 }}>{author}</span>
          {role && (
            <span style={{ fontSize: 24, opacity: 0.9, fontWeight: 500 }}>
              {role}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
