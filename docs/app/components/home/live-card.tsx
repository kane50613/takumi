"use client";

import { type ReactNode, useEffect, useId, useRef, useState } from "react";
import type { Node } from "takumi-js/helpers";
import type { LiveCardRequest, LiveCardResponse } from "./live-card-worker";

type Props = {
  node: Node;
  headline: string;
  image: string;
  width: number;
  height: number;
  intro: ReactNode;
  outro: ReactNode;
};

/**
 * The homepage's last section: a headline field that Takumi's WebAssembly build renders into the card on each
 * edit, in a worker that starts once the section comes near.
 */
export function LiveCard({ node, headline, image, width, height, intro, outro }: Props) {
  const inputId = useId();
  const section = useRef<HTMLDivElement>(null);
  const worker = useRef<Worker | undefined>(undefined);
  const pending = useRef<string | undefined>(undefined);
  const busy = useRef(false);
  const nextId = useRef(0);
  const [text, setText] = useState(headline);
  const [src, setSrc] = useState(image);
  const [status, setStatus] = useState("Rendered while the site built.");

  const send = (value: string) => {
    const current = worker.current;

    if (!current) {
      pending.current = value;
      return;
    }

    if (busy.current) {
      pending.current = value;
      return;
    }

    busy.current = true;
    pending.current = undefined;

    const request: LiveCardRequest = { type: "render", id: ++nextId.current, headline: value };

    current.postMessage(request);
  };

  const sendRef = useRef(send);

  sendRef.current = send;

  useEffect(() => {
    const element = section.current;
    let disposed = false;
    let url: string | undefined;

    const start = async () => {
      if (worker.current || disposed) {
        return;
      }

      const { default: LiveCardWorker } = await import("./live-card-worker?worker");

      if (disposed) {
        return;
      }

      const created = new LiveCardWorker();
      const setup: LiveCardRequest = { type: "setup", node, headline, width, height };

      created.onmessage = ({ data }: MessageEvent<LiveCardResponse>) => {
        busy.current = false;
        if (data.type === "result") {
          if (url) {
            URL.revokeObjectURL(url);
          }
          url = URL.createObjectURL(new Blob([data.image], { type: "image/webp" }));
          setSrc(url);
          setStatus(`Rendered in ${Math.max(1, Math.round(data.duration))} ms in this tab.`);
        } else {
          setStatus(`Could not render: ${data.message}`);
        }
        if (pending.current !== undefined) {
          sendRef.current(pending.current);
        }
      };
      created.postMessage(setup);
      worker.current = created;
      if (pending.current !== undefined) {
        setStatus("Loading the WebAssembly build…");
        sendRef.current(pending.current);
      }
    };

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          observer.disconnect();
          void start();
        }
      },
      { rootMargin: "50% 0px" },
    );

    if (element) {
      observer.observe(element);
    }

    return () => {
      disposed = true;
      observer.disconnect();
      worker.current?.terminate();
      worker.current = undefined;
      if (url) {
        URL.revokeObjectURL(url);
      }
    };
  }, [node, headline, width, height]);

  return (
    <div
      ref={section}
      className="grid items-center gap-x-16 gap-y-10 lg:grid-cols-[minmax(0,28rem)_minmax(0,1fr)]"
    >
      <div className="space-y-6">
        {intro}
        <div className="space-y-3">
          <label htmlFor={inputId} className="block text-lg font-[560]">
            Headline
          </label>
          <input
            id={inputId}
            value={text}
            maxLength={80}
            spellCheck={false}
            autoComplete="off"
            placeholder="Type a new headline"
            onChange={(event) => {
              setText(event.target.value);
              send(event.target.value);
            }}
            className="h-14 w-full rounded-md border bg-background px-4 text-xl outline-none transition-[border-color,box-shadow] duration-150 ease-out focus-visible:border-foreground/40 focus-visible:ring-[3px] focus-visible:ring-ring"
          />
          <p className="font-mono text-base text-muted-foreground">{status}</p>
        </div>
        {outro}
      </div>
      <img
        src={src}
        alt={`The card rendered with the headline “${text}”.`}
        width={width}
        height={height}
        loading="lazy"
        decoding="async"
        className="h-auto w-full rounded-md border"
      />
    </div>
  );
}
