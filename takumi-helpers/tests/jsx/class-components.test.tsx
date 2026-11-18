import { describe, expect, test } from "bun:test";
import { Component, memo, PureComponent } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { fromJsx } from "../../src/jsx";
import { serializeSvg } from "../../src/jsx/svg";

class Greeting extends Component<{ name: string }> {
  override render() {
    return <div>Hello {this.props.name}</div>;
  }
}

class Badge extends PureComponent<{ label?: string }, { shown: string }> {
  static defaultProps = { label: "new" };

  override state = { shown: "" };

  static getDerivedStateFromProps(props: { label?: string }) {
    return { shown: props.label?.toUpperCase() ?? "" };
  }

  override render() {
    return <span>{this.state.shown}</span>;
  }
}

class Dot extends Component {
  override render() {
    return <circle cx="5" cy="5" r="5" />;
  }
}

describe("class components", () => {
  test("a class component renders through its render method", async () => {
    const { node } = await fromJsx(<Greeting name="Takumi" />);

    expect(node).toMatchObject({ type: "text", text: "Hello Takumi" });
  });

  test("default props and derived state apply before render, as React mounts them", async () => {
    const { node } = await fromJsx(<Badge />);

    expect(node).toMatchObject({ type: "text", text: "NEW" });
  });

  test("a memoized class component renders", async () => {
    const Memoized = memo(Greeting);
    const { node } = await fromJsx(<Memoized name="memo" />);

    expect(node).toMatchObject({ type: "text", text: "Hello memo" });
  });

  test("a class component inside an svg serializes as React DOM writes it", () => {
    const svg = (
      <svg xmlns="http://www.w3.org/2000/svg">
        <Dot />
      </svg>
    );

    expect(serializeSvg(svg)).toBe(renderToStaticMarkup(svg));
  });
});
