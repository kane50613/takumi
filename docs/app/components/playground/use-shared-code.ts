import { useEffect, useState } from "react";
import { compressCode, decompressCode } from "~/playground/share";
import { defaultTemplate, templates } from "~/playground/templates";

export const DEFAULT_TEMPLATE = templates[0];

export function useSharedCode() {
  const [code, setCode] = useState<string>();
  const [searchParams, setSearchParams] = useState(() => {
    if (typeof window === "undefined") {
      return new URLSearchParams();
    }

    return new URLSearchParams(window.location.search);
  });

  useEffect(() => {
    const onPopState = () => {
      setSearchParams(new URLSearchParams(window.location.search));
    };

    window.addEventListener("popstate", onPopState);

    return () => {
      window.removeEventListener("popstate", onPopState);
    };
  }, []);

  const codeQuery = searchParams.get("code");
  const templateQuery = searchParams.get("template");
  const matchedTemplate = templates.find((template) => template.code === code);

  const replaceSearchParams = (updater: (current: URLSearchParams) => URLSearchParams) => {
    const next = updater(new URLSearchParams(window.location.search));
    const search = next.toString();
    const url = `${window.location.pathname}${search ? `?${search}` : ""}${window.location.hash}`;

    window.history.replaceState(window.history.state, "", url);
    setSearchParams(next);
  };

  useEffect(() => {
    if (code !== undefined) return;

    let cancelled = false;

    void (async () => {
      const templateCode = templates.find((template) => template.id === templateQuery)?.code;
      const initialCode = codeQuery
        ? await decompressCode(codeQuery)
        : (templateCode ?? DEFAULT_TEMPLATE.code);

      if (!cancelled) {
        setCode(initialCode);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [codeQuery, code, templateQuery]);

  useEffect(() => {
    if (!code) return;

    if (code === defaultTemplate) {
      replaceSearchParams((current) => {
        const next = new URLSearchParams(current);
        next.delete("code");
        next.delete("template");
        return next;
      });
      return;
    }

    if (matchedTemplate) {
      replaceSearchParams((current) => {
        const next = new URLSearchParams(current);
        next.delete("code");
        next.set("template", matchedTemplate.id);
        return next;
      });
      return;
    }

    const timer = setTimeout(() => {
      compressCode(code).then((base64) => {
        replaceSearchParams((current) => {
          const next = new URLSearchParams(current);
          next.delete("template");
          next.set("code", base64);
          return next;
        });
      });
    }, 500);

    return () => clearTimeout(timer);
  }, [code, matchedTemplate]);

  return { code, setCode, matchedTemplate };
}
