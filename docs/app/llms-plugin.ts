import { llmsPlugin } from "fumapress/plugins/llms.txt";
import type { AppShape, PressPlugin, RouteFns } from "fumapress";
import { AGENT_GUIDE } from "~/agent-guide";

/// `llmsPlugin` builds `llms.txt` from the page tree alone, so `AGENT_GUIDE` is spliced into
/// its handler instead.
export function agentGuidePlugin<C extends AppShape = AppShape>(): PressPlugin<C> {
  const base = llmsPlugin<C>();

  return {
    ...base,
    createPages(fns: RouteFns) {
      return base.createPages?.call(this, {
        ...fns,
        createApiIsomorphic(config) {
          if (config.path !== "/llms.txt") return fns.createApiIsomorphic(config);

          return fns.createApiIsomorphic({
            ...config,
            async handler(...args) {
              const response = await config.handler(...args);

              return new Response(AGENT_GUIDE + (await response.text()), {
                headers: { "Content-Type": "text/plain; charset=utf-8" },
              });
            },
          });
        },
      });
    },
  };
}
