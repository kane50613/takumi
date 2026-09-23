import { useEffect, useRef, useState } from "react";
import { compressCode, decompressCode } from "~/playground/share";
import { DEFAULT_TEMPLATE, templates } from "~/playground/templates";

function hashParams() {
  if (typeof window === "undefined") return new URLSearchParams();

  return new URLSearchParams(window.location.hash.slice(1));
}

// Links minted before the snippet moved into the fragment still carry it in the
// query string, where it reached the server on every visit.
function legacyQueryParams() {
  if (typeof window === "undefined") return new URLSearchParams();

  return new URLSearchParams(window.location.search);
}

export function useSharedCode() {
  const [code, setCode] = useState<string>();
  const [params, setParams] = useState(hashParams);
  // The URL loses its `code` parameter once the snippet is on screen, so where
  // it came from is remembered here instead of read back off the URL.
  const cameFromLink = useRef(false);

  useEffect(() => {
    const onHashChange = () => {
      setCode(undefined);
      setParams(hashParams());
    };

    window.addEventListener("hashchange", onHashChange);
    window.addEventListener("popstate", onHashChange);

    return () => {
      window.removeEventListener("hashchange", onHashChange);
      window.removeEventListener("popstate", onHashChange);
    };
  }, []);

  const legacy = legacyQueryParams();
  const codeQuery = params.get("code") ?? legacy.get("code");
  const templateQuery = params.get("template") ?? legacy.get("template");
  const matchedTemplate = templates.find((template) => template.code === code);

  const replaceParams = (update: (next: URLSearchParams) => void) => {
    const next = hashParams();

    update(next);

    const hash = next.toString();
    const url = `${window.location.pathname}${hash ? `#${hash}` : ""}`;

    window.history.replaceState(window.history.state, "", url);
    setParams(next);
  };

  useEffect(() => {
    if (code !== undefined) return;

    let cancelled = false;

    void (async () => {
      const templateCode = templates.find((template) => template.id === templateQuery)?.code;
      const initialCode = codeQuery
        ? await decompressCode(codeQuery).catch(() => DEFAULT_TEMPLATE.code)
        : (templateCode ?? DEFAULT_TEMPLATE.code);

      if (!cancelled) {
        cameFromLink.current = Boolean(codeQuery);
        setCode(initialCode);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [codeQuery, code, templateQuery]);

  useEffect(() => {
    if (!code) return;

    if (code === DEFAULT_TEMPLATE.code) {
      replaceParams((next) => {
        next.delete("code");
        next.delete("template");
      });
      return;
    }

    if (matchedTemplate) {
      replaceParams((next) => {
        next.delete("code");
        next.set("template", matchedTemplate.id);
      });
      return;
    }

    const timer = setTimeout(() => {
      compressCode(code).then((base64) => {
        replaceParams((next) => {
          next.delete("template");
          next.set("code", base64);
        });
      });
    }, 500);

    return () => clearTimeout(timer);
  }, [code, matchedTemplate]);

  return { code, setCode, matchedTemplate, isShared: cameFromLink.current };
}
