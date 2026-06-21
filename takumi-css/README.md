# takumi-css

<!-- cargo-rdme start -->

CSS parsing and computed-style layer for takumi.

Holds the (cold) CSS parsing, cascade, and value types so they can be
compiled independently from the hot rendering paths in `takumi`. Selector
_matching_ against the node tree lives in `takumi` (`layout::matching`),
not here, keeping this crate free of any node/render dependency.

<!-- cargo-rdme end -->
