"use client";

import { Dialog } from "@base-ui/react/dialog";
import type { SharedProps } from "fumadocs-ui/contexts/search";
import { lazy, Suspense, useEffect, useState } from "react";

const loadResults = () => import("./results");
const Results = lazy(loadResults);

/**
 * Fumapress's static Orama search dialog, loaded on first use instead of with every page. Until
 * then an empty dialog root answers the search triggers, which open it through `dialogHandle`.
 */
export function SearchDialog(props: SharedProps) {
  const [used, setUsed] = useState(props.open);

  if (props.open && !used) {
    setUsed(true);
  }

  useEffect(() => {
    const prefetch = (event: Event) => {
      if (
        event.target instanceof Element &&
        event.target.closest("[data-search],[data-search-full]")
      ) {
        void loadResults();
      }
    };

    document.addEventListener("pointerover", prefetch, { passive: true });
    document.addEventListener("focusin", prefetch);
    return () => {
      document.removeEventListener("pointerover", prefetch);
      document.removeEventListener("focusin", prefetch);
    };
  }, []);

  if (!used) {
    return (
      <Dialog.Root
        open={props.open}
        onOpenChange={props.onOpenChange}
        handle={props.dialogHandle}
      />
    );
  }

  return (
    <Suspense fallback={<LoadingDialog {...props} />}>
      <Results {...props} />
    </Suspense>
  );
}

/** The search dialog's frame while its results chunk loads. */
function LoadingDialog({ open, onOpenChange, dialogHandle }: SharedProps) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange} handle={dialogHandle}>
      <Dialog.Portal>
        <Dialog.Backdrop className="fixed inset-0 z-50 bg-fd-overlay backdrop-blur-xs" />
        <Dialog.Popup
          aria-busy
          className="fixed top-4 left-1/2 z-50 w-[calc(100%-1rem)] max-w-screen-sm -translate-x-1/2 rounded-xl border bg-fd-popover p-4 text-fd-muted-foreground shadow-2xl md:top-[calc(50%-250px)]"
        >
          <Dialog.Title className="text-sm">Loading search…</Dialog.Title>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
