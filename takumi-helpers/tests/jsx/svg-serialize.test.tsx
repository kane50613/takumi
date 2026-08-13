import { expect, test } from "bun:test";
import { forwardRef, memo, StrictMode } from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { serializeSvg } from "../../src/jsx/svg";

test("serializeSvg matches react-dom server output for SVG", () => {
  const component = (
    <svg width="60" height="60" viewBox="0 0 180 180" xmlns="http://www.w3.org/2000/svg">
      <title>Logo</title>
      <circle cx="90" cy="90" r="86" fill="url(#logo-iconGradient)" />
      <defs>
        <linearGradient id="logo-iconGradient" gradientTransform="rotate(45)">
          <stop offset="45%" stopColor="black" />
          <stop offset="100%" stopColor="white" />
        </linearGradient>
      </defs>
    </svg>
  );

  const expected = renderToStaticMarkup(component);
  const actual = serializeSvg(component);

  expect(actual).toBe(expected);
});

test("serializeSvg handles camelCase SVG props and boolean attributes", () => {
  const component = (
    <svg xmlns="http://www.w3.org/2000/svg">
      <defs>
        <filter id="f" colorInterpolationFilters="sRGB">
          <feDropShadow dx="0" dy="0" stdDeviation="4" floodColor="white" floodOpacity="1" />
        </filter>
      </defs>
      <rect x="0" y="0" width="10" height="10" fillOpacity={0.5} focusable />
    </svg>
  );

  const expected = renderToStaticMarkup(component);
  const actual = serializeSvg(component);

  expect(actual).toBe(expected);
});

test("serializeSvg preserves style objects and converts camelCase style keys", () => {
  const component = (
    <svg xmlns="http://www.w3.org/2000/svg">
      <rect style={{ strokeWidth: 2, strokeDasharray: "4 2" }} />
    </svg>
  );

  const expected = renderToStaticMarkup(component);
  const actual = serializeSvg(component);

  expect(actual).toBe(expected);
});

test("serializeSvg emits fragment children, including from components", () => {
  const Bars = () => (
    <>
      <rect x="0" width="10" height="10" />
      <rect x="20" width="10" height="10" />
    </>
  );

  const component = (
    <svg xmlns="http://www.w3.org/2000/svg">
      <>
        <circle cx="5" cy="5" r="5" />
      </>
      <Bars />
      <StrictMode>
        <rect x="40" />
      </StrictMode>
    </svg>
  );

  const expected = renderToStaticMarkup(component);
  const actual = serializeSvg(component);

  expect(actual).toBe(expected);
});

test("serializeSvg renders memo and forwardRef children", () => {
  const Memoized = memo(() => <rect x="0" width="10" height="10" />);
  const Forwarded = forwardRef<SVGCircleElement>((_props, ref) => (
    <circle ref={ref} cx="5" cy="5" r="5" />
  ));

  const component = (
    <svg xmlns="http://www.w3.org/2000/svg">
      <Memoized />
      <Forwarded />
    </svg>
  );

  const expected = renderToStaticMarkup(component);
  const actual = serializeSvg(component);

  expect(actual).toBe(expected);
});

test("serializeSvg adds xmlns when not provided", () => {
  const component = (
    <svg>
      <rect x="0" y="0" width="10" height="10" />
    </svg>
  );

  const expected = renderToStaticMarkup(
    <svg xmlns="http://www.w3.org/2000/svg">
      <rect x="0" y="0" width="10" height="10" />
    </svg>,
  );

  const actual = serializeSvg(component);

  expect(actual).toBe(expected);
});
