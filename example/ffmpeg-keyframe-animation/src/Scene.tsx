import type { Keyframes } from "@takumi-rs/core";
import type { ThemedToken } from "shiki";
import { createHighlighter } from "shiki";

const demoCode = `
@keyframes pulse {
  0%, 100% {
    transform: scale(1);
    opacity: 0.7;
  }
  50% {
    transform: scale(1.1);
    opacity: 1;
  }
}
`.trim();

export const getHighlighterTokens = async () => {
  const highlighter = await createHighlighter({
    themes: ["github-dark-default"],
    langs: ["css"],
  });

  return highlighter.codeToTokens(demoCode, {
    lang: "css",
    theme: "github-dark-default",
  });
};

export const keyframes: Keyframes = {
  windowReveal: {
    from: {
      transform: "translateY(20px) scale(0.95)",
      opacity: 0,
    },
    to: {
      transform: "translateY(0) scale(1)",
      opacity: 1,
    },
  },
  windowExit: {
    from: {
      transform: "translateY(0) scale(1)",
      opacity: 1,
    },
    to: {
      transform: "translateY(-20px) scale(0.95)",
      opacity: 0,
    },
  },
  textReveal: {
    from: {
      transform: "translateY(10px)",
      opacity: 0,
    },
    to: {
      transform: "translateY(0)",
      opacity: 1,
    },
  },
};

export const Scene = ({
  tokens,
  showPlayButton = false,
}: {
  tokens: { tokens: ThemedToken[][] };
  showPlayButton?: boolean;
}) => {
  let tokenAnimationIndex = 0;

  return (
    <main tw="relative flex h-full w-full items-center justify-center overflow-hidden">
      <img
        src="background.jpg"
        tw="absolute inset-0 h-full w-full object-cover"
        alt="Abstract wavy background"
      />
      <div tw="absolute inset-0 bg-black/10" />
      <div
        tw="flex font-mono flex-col items-start overflow-hidden rounded-xl bg-black/40 p-8 ring-1 ring-white/30 shadow-2xl shadow-black/50"
        style={{
          backdropFilter: "blur(24px)",
          fontSize: "26px",
          width: "720px",
          animation:
            "windowReveal 0.6s cubic-bezier(0.34, 1.56, 0.64, 1) both, windowExit 0.4s cubic-bezier(0.36, 0, 0.66, -0.56) 3.5s forwards",
        }}
      >
        <div tw="mb-8 flex gap-2.5">
          <div tw="h-3.5 w-3.5 rounded-full bg-[#ff5f56]" />
          <div tw="h-3.5 w-3.5 rounded-full bg-[#ffbd2e]" />
          <div tw="h-3.5 w-3.5 rounded-full bg-[#27c93f]" />
        </div>
        <div tw="flex flex-col gap-1.5 whitespace-pre-wrap pl-2">
          {tokens.tokens.map((line: ThemedToken[], i: number) => (
            <div key={i} tw="flex">
              {line.map((token: ThemedToken, j: number) => {
                const delay = 0.3 + tokenAnimationIndex * 0.025;
                tokenAnimationIndex += 1;

                return (
                  <span
                    key={j}
                    style={{
                      color: token.color,
                      opacity: 0,
                      animation: `textReveal 0.15s ease-out ${delay}s forwards`,
                    }}
                  >
                    {token.content}
                  </span>
                );
              })}
            </div>
          ))}
        </div>
        <img
          src="logo.svg"
          alt="Logo"
          tw="absolute"
          style={{
            width: 64,
            height: 64,
            bottom: 40,
            right: 40,
          }}
        />
      </div>
      {showPlayButton && (
        <div
          tw="absolute inset-0 flex items-center justify-center bg-black/30"
          style={{ backdropFilter: "blur(2px)" }}
        >
          <svg width="256" height="256" viewBox="0 0 24 24" fill="white">
            <title>Play Animation</title>
            <path d="M8 5v14l11-7z" />
          </svg>
        </div>
      )}
    </main>
  );
};
