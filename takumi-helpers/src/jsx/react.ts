export interface ReactRuntime {
  internals: {
    H: ReactDispatcher | null;
  };
}

interface ReactDispatcher {
  useContext(context: { _currentValue: unknown }): unknown;
}

type ReactModuleLike = {
  __CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE?: unknown;
};

export const REACT_CONTEXT_TYPE = Symbol.for("react.context");
export const REACT_CONSUMER_TYPE = Symbol.for("react.consumer");

export function isReactContext(value: unknown): value is {
  $$typeof: symbol;
  _currentValue: unknown;
} {
  return typeof value === "object" && value !== null && "$$typeof" in value;
}

export function isReactContextProvider(element: { type: unknown }): boolean {
  return isReactContext(element.type) && element.type.$$typeof === REACT_CONTEXT_TYPE;
}

export function isReactContextConsumer(element: { type: unknown }): element is {
  type: {
    $$typeof: symbol;
    _context: unknown;
  };
} {
  return (
    typeof element.type === "object" &&
    element.type !== null &&
    "$$typeof" in element.type &&
    element.type.$$typeof === REACT_CONSUMER_TYPE &&
    "_context" in element.type
  );
}

let reactRuntimePromise: Promise<ReactRuntime | null> | undefined;

export function getReactRuntime(
  currentRuntime: Promise<ReactRuntime | null> | null,
): Promise<ReactRuntime | null> {
  if (currentRuntime) return currentRuntime;

  reactRuntimePromise ??= import("react")
    .then((module) => {
      const candidate = (module.default ?? module) as ReactModuleLike;
      const internals = candidate.__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE;

      if (!internals || typeof internals !== "object" || !("H" in internals)) {
        return null;
      }

      return {
        internals: internals as ReactRuntime["internals"],
      };
    })
    .catch(() => null);

  return reactRuntimePromise;
}

export function withContextValue(
  contextValues: ReadonlyMap<object, unknown>,
  context: object,
  value: unknown,
): ReadonlyMap<object, unknown> {
  const nextContextValues = new Map(contextValues);
  nextContextValues.set(context, value);

  return nextContextValues;
}

export function createContextDispatcher(
  contextValues: ReadonlyMap<object, unknown>,
): ReactDispatcher {
  return {
    useContext(context) {
      if (contextValues.has(context)) {
        return contextValues.get(context);
      }

      return context._currentValue;
    },
  };
}
