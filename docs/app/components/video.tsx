import type { ComponentPropsWithoutRef } from "react";
import { cn } from "~/lib/utils";

export function Video(props: ComponentPropsWithoutRef<"video">) {
  return (
    <video
      preload="auto"
      autoPlay
      muted
      loop
      playsInline
      {...props}
      className={cn("rounded-xl border bg-fd-background w-full", props.className)}
    />
  );
}
