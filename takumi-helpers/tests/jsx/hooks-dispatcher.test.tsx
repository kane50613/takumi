import { describe, expect, test } from "bun:test";
import {
  createContext,
  use,
  useContext,
  useId,
  useMemo,
  useReducer,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { fromJsx } from "../../src/jsx";

describe("hook resolution through the installed dispatcher", () => {
  test("resolves state, memo, ref, reducer, and store hooks", async () => {
    const Component = () => {
      const [count] = useState(() => 3);
      const doubled = useMemo(() => count * 2, [count]);
      const ref = useRef("ref");
      const [word] = useReducer((state: string) => state, "reduced");
      const stored = useSyncExternalStore(
        () => () => {},
        () => "client",
        () => "server",
      );

      return (
        <p>
          {count} {doubled} {ref.current} {word} {stored}
        </p>
      );
    };

    const { node } = await fromJsx(<Component />);

    expect(node).toMatchObject({
      type: "text",
      text: "3 6 ref reduced server",
    });
  });

  test("useId is deterministic per render", async () => {
    const Component = () => {
      const first = useId();
      const second = useId();

      return (
        <p>
          {first} {second}
        </p>
      );
    };

    const [a, b] = await Promise.all([fromJsx(<Component />), fromJsx(<Component />)]);

    expect(a.node).toMatchObject({ type: "text", text: ":t0: :t1:" });
    expect(a.node).toEqual(b.node);
  });

  test("use() suspends on a pending promise and replays", async () => {
    const value = Promise.resolve("resolved");
    const Component = () => <p>{use(value)}</p>;

    const { node } = await fromJsx(<Component />);

    expect(node).toMatchObject({ type: "text", text: "resolved" });
  });

  test("useContext reads nested provider values natively", async () => {
    const Greeting = createContext("default");

    const Inner = () => <p>{useContext(Greeting)}</p>;

    const { node } = await fromJsx(
      <div>
        <Inner />
        <Greeting.Provider value="outer">
          <Inner />
          <Greeting.Provider value="inner">
            <Inner />
          </Greeting.Provider>
        </Greeting.Provider>
      </div>,
    );

    expect(node).toMatchObject({
      type: "container",
      children: [
        { type: "text", text: "default" },
        { type: "text", text: "outer" },
        { type: "text", text: "inner" },
      ],
    });
  });

  test("consumer render prop receives the provided value", async () => {
    const Greeting = createContext("default");

    const { node } = await fromJsx(
      <Greeting.Provider value="provided">
        <Greeting.Consumer>{(value) => <p>{value}</p>}</Greeting.Consumer>
      </Greeting.Provider>,
    );

    expect(node).toMatchObject({ type: "text", text: "provided" });
  });

  test("hooks and context compose across component boundaries", async () => {
    const Theme = createContext("light");

    const Label = ({ suffix }: { suffix: string }) => {
      const theme = use(Theme);
      const label = useMemo(() => `${theme}-${suffix}`, [theme, suffix]);

      return <p tw="font-bold">{label}</p>;
    };

    const { node } = await fromJsx(
      <Theme.Provider value="dark">
        <Label suffix="mode" />
      </Theme.Provider>,
    );

    expect(node).toMatchObject({
      type: "text",
      text: "dark-mode",
      tw: "font-bold",
    });
  });
});
