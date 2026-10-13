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
    <Suspense fallback={null}>
      <Results {...props} />
    </Suspense>
  );
}
