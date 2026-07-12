import type { ReactNode } from "react";

/**
 * Hooks read the active dispatcher from React's shared internals; only a
 * renderer installs one. Installing a minimal server-semantics dispatcher
 * (initial state, no effects) around synchronous component calls resolves hooks
 * without react-dom/server, the same way react-ssr-prepass does:
 * https://github.com/FormidableLabs/react-ssr-prepass
 */

// React 19 and 18 internals keys; both shapes are feature-detected below.
const CLIENT_INTERNALS = "__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE";
const SECRET_INTERNALS = "__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED";

export interface RenderEnv {
  contexts: ReadonlyMap<unknown, unknown>;
  ids: { current: number };
}

interface DispatcherSlot {
  get(): unknown;
  set(value: unknown): void;
}

interface TrackedThenable extends PromiseLike<unknown> {
  status?: string;
  value?: unknown;
  reason?: unknown;
}

let slotPromise: Promise<DispatcherSlot | null> | undefined;

export function getProperty(object: unknown, key: string): unknown {
  return typeof object === "object" && object !== null && key in object
    ? (object as Record<string, unknown>)[key]
    : undefined;
}

function isThenable(value: unknown): value is TrackedThenable {
  return (
    typeof value === "object" &&
    value !== null &&
    "then" in value &&
    typeof value.then === "function"
  );
}

function resolveSlot(react: unknown): DispatcherSlot | null {
  const client = getProperty(react, CLIENT_INTERNALS);
  if (typeof client === "object" && client !== null && "H" in client) {
    const internals = client as { H: unknown };

    return {
      get: () => internals.H,
      set: (value) => {
        internals.H = value;
      },
    };
  }

  const dispatcherRef = getProperty(getProperty(react, SECRET_INTERNALS), "ReactCurrentDispatcher");
  if (typeof dispatcherRef === "object" && dispatcherRef !== null && "current" in dispatcherRef) {
    const ref = dispatcherRef as { current: unknown };

    return {
      get: () => ref.current,
      set: (value) => {
        ref.current = value;
      },
    };
  }

  return null;
}

function getSlot(): Promise<DispatcherSlot | null> {
  slotPromise ??= import("react")
    .then((module) => resolveSlot(module.default ?? module))
    .catch(() => null);

  return slotPromise;
}

export function readContext(env: RenderEnv, context: unknown): unknown {
  return env.contexts.has(context)
    ? env.contexts.get(context)
    : getProperty(context, "_currentValue");
}

const noop = () => {};

function createDispatcher(env: RenderEnv): Record<string, unknown> {
  const read = (context: unknown) => readContext(env, context);

  return {
    readContext: read,
    useContext: read,
    use: (usable: unknown): unknown => {
      if (isThenable(usable)) {
        if (usable.status === "fulfilled") return usable.value;
        if (usable.status === "rejected") throw usable.reason;
        throw usable;
      }

      return read(usable);
    },
    useState: (initial: unknown) => [typeof initial === "function" ? initial() : initial, noop],
    useReducer: (_reducer: unknown, initialArg: unknown, init?: (arg: unknown) => unknown) => [
      init ? init(initialArg) : initialArg,
      noop,
    ],
    useMemo: (factory: () => unknown) => factory(),
    useCallback: (callback: unknown) => callback,
    useRef: (initial: unknown) => ({ current: initial }),
    useEffect: noop,
    useLayoutEffect: noop,
    useInsertionEffect: noop,
    useImperativeHandle: noop,
    useDebugValue: noop,
    useDeferredValue: (value: unknown) => value,
    useTransition: () => [false, noop],
    useOptimistic: (state: unknown) => [state, noop],
    useActionState: (_action: unknown, initial: unknown) => [initial, noop, false],
    useSyncExternalStore: (
      _subscribe: unknown,
      getSnapshot: () => unknown,
      getServerSnapshot?: () => unknown,
    ) => (getServerSnapshot ?? getSnapshot)(),
    useId: () => `:t${env.ids.current++}:`,
    useCacheRefresh: () => noop,
    useHostTransitionStatus: () => ({ pending: false, data: null, method: null, action: null }),
  };
}

// Bounds `use()` replays; a component creating a fresh promise every render
// would otherwise replay forever (React warns on the same pattern).
const MAX_THENABLE_REPLAYS = 64;

/**
 * Calls a function component with the dispatcher installed, replaying the call
 * when `use()` throws a pending thenable. Without React (or with unrecognized
 * internals) the component is called bare, so hook-free components still work.
 */
export async function callWithDispatcher(
  component: (props: unknown) => ReactNode,
  props: unknown,
  env: RenderEnv,
): Promise<ReactNode> {
  const slot = await getSlot();
  if (!slot) return component(props);

  for (let replays = 0; ; replays++) {
    const previous = slot.get();
    slot.set(createDispatcher(env));

    try {
      return component(props);
    } catch (thrown) {
      if (!isThenable(thrown) || replays >= MAX_THENABLE_REPLAYS) throw thrown;

      await settleThenable(thrown);
    } finally {
      slot.set(previous);
    }
  }
}

// Stamps resolution onto the thenable (React's own convention) so the replayed
// `use()` can read it synchronously.
async function settleThenable(thenable: TrackedThenable): Promise<void> {
  try {
    thenable.value = await thenable;
    thenable.status = "fulfilled";
  } catch (reason) {
    thenable.reason = reason;
    thenable.status = "rejected";
  }
}
