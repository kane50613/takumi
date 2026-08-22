import { cn } from "~/lib/utils";

/** Covers the pane until the editor and the renderer are both up. */
export function LoadingScreen({ done }: { done?: boolean }) {
  return (
    <div
      aria-hidden={done}
      className={cn(
        "playground-loading absolute inset-0 z-50 grid place-items-center bg-background",
        done && "playground-loading-done pointer-events-none",
      )}
    >
      <div className="grid justify-items-center gap-4">
        <img
          src="/sticker.svg"
          alt="Takumi"
          height={210}
          width={530}
          className="playground-breathe h-10 w-fit"
        />
        <span className="playground-breathe text-sm text-fd-muted-foreground">
          Initializing Takumi.
        </span>
      </div>
    </div>
  );
}
