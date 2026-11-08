---
packages:
  takumi-core: minor
---

# CSS math functions

Lengths, numbers, and angles now take every math function of CSS Values 4: `min()`, `max()`, `clamp()`, `round()`, `mod()`, `rem()`, `abs()`, `sign()`, `hypot()`, the trigonometric functions, `pow()`, `sqrt()`, `log()`, and `exp()`. They nest inside `calc()` and each other, and `calc()` takes parentheses. A function that mixes a percentage with another unit, such as `min(200px, 50%)`, resolves once layout knows the percentage basis, as in a browser. Division by zero gives infinity, which clamps to the largest length, and a value divided by one of its own type gives a number. `Length::Math` holds such a function.
