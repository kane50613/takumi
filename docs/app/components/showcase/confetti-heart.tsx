"use client";

import JSConfetti from "js-confetti";
import { Heart } from "lucide-react";
import { useCallback, useRef } from "react";

export function ConfettiHeart() {
  const confettiRef = useRef<JSConfetti | null>(null);

  const onConfetti = useCallback((e: React.MouseEvent<HTMLButtonElement>) => {
    if (!confettiRef.current) {
      confettiRef.current = new JSConfetti();
    }

    const rect = e.currentTarget.getBoundingClientRect();
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;

    confettiRef.current.addConfettiAtPosition({
      emojis: ["❤️", "🪓", "🎨", "✨"],
      emojiSize: 40,
      confettiNumber: 30,
      confettiDispatchPosition: { x, y },
    });
  }, []);

  return (
    <button
      type="button"
      onClick={onConfetti}
      className="relative flex items-center justify-center w-20 h-20 transition-transform active:scale-95 cursor-pointer outline-none bg-background rounded-full border border-border/50 shadow-[0_0_30px_-5px_--theme(--color-primary/0.3)] backdrop-blur-sm group-hover:border-primary/40 group-hover:shadow-[0_0_40px_-5px_--theme(--color-primary/0.5)] z-10"
      aria-label="Celebrate"
    >
      <Heart className="w-8 h-8 text-primary fill-primary/20 group-hover:fill-primary transition-all duration-300" />
    </button>
  );
}
